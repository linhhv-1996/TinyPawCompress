use image::{DynamicImage, ImageBuffer, Luma, Rgb};
use lopdf::{Document, Object, ObjectId};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use turbojpeg::{Compressor, Decompressor, Image as TurboImage, PixelFormat, Subsamp};

pub struct PdfCompressOptions {
    pub profile: String,
    pub grayscale: bool,
    pub strip_meta: bool,
}

// KHẮC PHỤC 4: Dùng thread_local để tái sử dụng Context của thư viện C (TurboJPEG)
// Mỗi luồng của Rayon sẽ chỉ khởi tạo Compressor/Decompressor 1 lần duy nhất thay vì tạo mới cho từng bức ảnh.
thread_local! {
    static DECOMPRESSOR: std::cell::RefCell<Result<Decompressor, ()>> = 
        std::cell::RefCell::new(Decompressor::new().map_err(|_| ()));
    static COMPRESSOR: std::cell::RefCell<Result<Compressor, ()>> = 
        std::cell::RefCell::new(Compressor::new().map_err(|_| ()));
}

pub fn compress_pdf<P: AsRef<Path>>(
    input_path: P,
    output_path: P,
    options: PdfCompressOptions,
    cancel_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let output_path = output_path.as_ref();
    let temp_path = output_path.with_extension("tmp");

    let mut doc = Document::load(input_path).map_err(|e| format!("Không đọc được PDF: {}", e))?;

    let jpeg_quality = match options.profile.as_str() {
        "screen" => 40,
        "ebook" => 65,
        "printer" => 85,
        _ => 65,
    } as i32;

    // 1. Loại bỏ Metadata (nếu có)
    if options.strip_meta {
        doc.trailer.remove(b"Info");
        let mut meta_ids = vec![];
        for (id, obj) in &doc.objects {
            if let Object::Stream(s) = obj {
                if s.dict.get(b"Type").and_then(|o| o.as_name()).unwrap_or(b"") == b"Metadata" {
                    meta_ids.push(*id);
                }
            }
        }
        for id in meta_ids {
            doc.objects.remove(&id);
        }
    }

    // 2. Trích xuất objects để xử lý song song bằng Rayon
    let objects = std::mem::take(&mut doc.objects);

    let processed_objects: BTreeMap<ObjectId, Object> = objects
        .into_par_iter()
        .map(|(object_id, mut object)| {
            if cancel_flag.load(Ordering::Relaxed) {
                return (object_id, object);
            }

            if let Object::Stream(ref mut stream) = object {
                let is_image = stream
                    .dict
                    .get(b"Subtype")
                    .and_then(|obj| obj.as_name())
                    .unwrap_or(b"")
                    == b"Image";
                
                if !is_image {
                    return (object_id, object);
                }

                let width_orig = stream.dict.get(b"Width").and_then(|o| o.as_i64()).unwrap_or(0) as u32;
                let height_orig = stream.dict.get(b"Height").and_then(|o| o.as_i64()).unwrap_or(0) as u32;

                // KHẮC PHỤC 3: Bỏ qua các hình ảnh quá nhỏ (như icon, dot, separator) để tiết kiệm CPU vô ích
                if width_orig < 64 || height_orig < 64 || stream.content.len() < 5120 {
                    return (object_id, object);
                }

                let is_jpeg = match stream.dict.get(b"Filter") {
                    Ok(Object::Name(n)) => *n == b"DCTDecode" || *n == b"JPXDecode",
                    Ok(Object::Array(arr)) => arr.iter().any(|obj| {
                        let n = obj.as_name().unwrap_or(b"");
                        n == b"DCTDecode" || n == b"JPXDecode"
                    }),
                    _ => false,
                };

                // KHẮC PHỤC 1: Xóa bỏ hoàn toàn .clone(). Dùng reference &[u8] để đọc trực tiếp.
                // Chỉ cấp phát dữ liệu sở hữu mới (Owned Data) khi buộc phải giải nén Raw Stream.
                let decompressed_data_owned;
                let data_slice: &[u8] = if is_jpeg {
                    &stream.content
                } else {
                    match stream.decompressed_content() {
                        Ok(data) => {
                            decompressed_data_owned = data;
                            &decompressed_data_owned
                        }
                        Err(_) => return (object_id, object),
                    }
                };

                let is_gray_original = match stream.dict.get(b"ColorSpace") {
                    Ok(Object::Name(n)) => *n == b"DeviceGray" || *n == b"CalGray",
                    Ok(Object::Array(arr)) => {
                        arr.get(0).and_then(|o| o.as_name().ok()).unwrap_or(b"") == b"DeviceGray"
                    }
                    _ => false,
                };

                let is_output_gray = options.grayscale || is_gray_original;
                
                let mut turbo_pixels: Option<(u32, u32, Vec<u8>, PixelFormat, Vec<u8>, usize, Subsamp)> = None;

                // --- PATH A: GIẢI MÃ BẰNG TURBOJPEG (NHANH) ---
                if is_jpeg {
                    DECOMPRESSOR.with(|dec_cell| {
                        if let Ok(ref mut decompressor) = *dec_cell.borrow_mut() {
                            let format = if is_output_gray { PixelFormat::GRAY } else { PixelFormat::RGB };
                            if let Ok(header) = decompressor.read_header(data_slice) {
                                let w = header.width;
                                let h = header.height;
                                let pitch = if is_output_gray { w } else { w * 3 };
                                
                                let mut pixels = vec![0u8; h * pitch];

                                let dest_image = TurboImage {
                                    pixels: pixels.as_mut_slice(),
                                    width: w,
                                    pitch,
                                    height: h,
                                    format,
                                };

                                if decompressor.decompress(data_slice, dest_image).is_ok() {
                                    let pdf_color_space = if is_output_gray { b"DeviceGray".to_vec() } else { b"DeviceRGB".to_vec() };
                                    let subsamp = if is_output_gray { Subsamp::Gray } else { Subsamp::Sub2x2 };
                                    turbo_pixels = Some((w as u32, h as u32, pixels, format, pdf_color_space, pitch, subsamp));
                                }
                            }
                        }
                    });
                }

                // --- PATH B: GIẢI MÃ DỰ PHÒNG TỐI ƯU ---
                if turbo_pixels.is_none() {
                    let expected_rgb = (width_orig * height_orig * 3) as usize;
                    let expected_gray = (width_orig * height_orig) as usize;

                    let is_raw_rgb = width_orig > 0 && height_orig > 0 && data_slice.len() == expected_rgb;
                    let is_raw_gray = width_orig > 0 && height_orig > 0 && data_slice.len() == expected_gray;

                    // KHẮC PHỤC 2: Loại bỏ hoàn toàn image::load_from_memory.
                    // Chỉ nạp dữ liệu vào Buffer khi đã chắc chắn dung lượng mảng khớp với điểm ảnh (tránh Panic và nghẽn CPU)
                    if is_raw_rgb {
                        if let Some(dyn_img) = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width_orig, height_orig, data_slice.to_vec()).map(DynamicImage::ImageRgb8) {
                            let (w, h, raw, fmt, cs, pt, sub) = if is_output_gray {
                                let gray = dyn_img.into_luma8();
                                let (w, h) = (gray.width(), gray.height());
                                (w, h, gray.into_raw(), PixelFormat::GRAY, b"DeviceGray".to_vec(), w as usize, Subsamp::Gray)
                            } else {
                                let rgb = dyn_img.into_rgb8();
                                let (w, h) = (rgb.width(), rgb.height());
                                (w, h, rgb.into_raw(), PixelFormat::RGB, b"DeviceRGB".to_vec(), (w * 3) as usize, Subsamp::Sub2x2)
                            };
                            turbo_pixels = Some((w, h, raw, fmt, cs, pt, sub));
                        }
                    } else if is_raw_gray {
                        if let Some(dyn_img) = ImageBuffer::<Luma<u8>, Vec<u8>>::from_raw(width_orig, height_orig, data_slice.to_vec()).map(DynamicImage::ImageLuma8) {
                            let gray = dyn_img.into_luma8();
                            let (w, h) = (gray.width(), gray.height());
                            turbo_pixels = Some((w, h, gray.into_raw(), PixelFormat::GRAY, b"DeviceGray".to_vec(), w as usize, Subsamp::Gray));
                        }
                    }
                }

                // --- THỰC HIỆN NÉN LẠI ---
                if let Some((w, h, raw_pixels, pixel_format, pdf_color_space, pitch, subsamp)) = turbo_pixels {
                    COMPRESSOR.with(|comp_cell| {
                        if let Ok(ref mut compressor) = *comp_cell.borrow_mut() {
                            let _ = compressor.set_quality(jpeg_quality);
                            let _ = compressor.set_subsamp(subsamp);

                            let turbo_image = TurboImage {
                                pixels: raw_pixels.as_slice(),
                                width: w as usize,
                                pitch,
                                height: h as usize,
                                format: pixel_format,
                            };

                            if let Ok(compressed_image_data) = compressor.compress_to_vec(turbo_image) {
                                stream.content = compressed_image_data;
                                
                                stream.dict.set("Filter".as_bytes().to_vec(), Object::Name("DCTDecode".as_bytes().to_vec()));
                                stream.dict.remove(b"DecodeParms");
                                stream.dict.set("ColorSpace".as_bytes().to_vec(), Object::Name(pdf_color_space));
                                stream.dict.set("BitsPerComponent".as_bytes().to_vec(), Object::Integer(8));
                            }
                        }
                    });
                }
            }
            (object_id, object)
        })
        .collect();

    doc.objects = processed_objects;

    if cancel_flag.load(Ordering::Relaxed) {
        let _ = fs::remove_file(&temp_path);
        return Err("Cancelled".to_string());
    }

    doc.prune_objects();
    doc.save(&temp_path).map_err(|e| format!("Lỗi lưu PDF: {}", e))?;
    fs::rename(&temp_path, output_path).map_err(|e| format!("Lỗi đổi tên file: {}", e))?;

    Ok(())
}
