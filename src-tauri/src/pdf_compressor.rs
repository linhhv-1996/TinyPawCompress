use image::{imageops::FilterType, DynamicImage, GrayImage, RgbImage};
use lopdf::{Document, Object, SaveOptions};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use rayon::ThreadPoolBuilder;
use std::fs;
use std::io::Cursor;

// Struct chứa dữ liệu thô đẩy sang luồng song song
struct ImageTask {
    id: (u32, u16),
    img_bytes: Vec<u8>,
    filter: String,
    width: u32,
    height: u32,
    color_space: String,
}

// Struct chứa kết quả trả về từ luồng sau khi xử lý xong
struct ProcessedImage {
    id: (u32, u16),
    compressed_bytes: Vec<u8>,
    width: u32,
    height: u32,
    is_gray: bool,
}

pub fn compress_pdf(
    input_path: &str,
    output_path: &str,
    profile: &str,
    grayscale: bool,
    strip_meta: bool,
    unlock_pdf: bool,
    password: &str,
    cancel_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    // 1. TẢI VÀ GIẢI MÃ BẰNG LOPDF
    let mut doc = if !password.is_empty() {
        Document::load_with_password(input_path, password)
            .map_err(|_| "Incorrect password or unable to open PDF file!".to_string())?
    } else {
        Document::load(input_path)
            .map_err(|e| format!("Error reading PDF file: {}", e))?
    };

    // 2. KIỂM TRA TRẠNG THÁI MÃ HÓA
    if doc.is_encrypted() {
        return Err("This PDF is password protected. Please enter the correct password.".to_string());
    }

    // 3. XỬ LÝ LOGIC UNLOCK
    if unlock_pdf {
        doc.encryption_state = None;
        doc.trailer.remove(b"Encrypt");
    }

    let (quality, max_size) = match profile {
        "screen" => (40, 800),
        "ebook" => (65, 1280),
        "printer" => (85, 2048),
        _ => (60, 1280),
    };

    if strip_meta {
        doc.trailer.remove(b"Info");
    }

    // 4. KHỞI TẠO THREAD POOL VÀ NÉN ẢNH
    let pool = ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .map_err(|e| format!("Failed to initialize ThreadPool: {}", e))?;

    let object_ids: Vec<_> = doc.objects.keys().copied().collect();
    let mut image_ids = Vec::new();

    for id in object_ids {
        if let Ok(Object::Stream(stream)) = doc.get_object(id) {
            let subtype_match = stream.dict.get(b"Subtype").and_then(|obj| obj.as_name()).unwrap_or(b"") == b"Image" as &[u8];
            if subtype_match {
                image_ids.push(id);
            }
        }
    }

    for chunk in image_ids.chunks(4) {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        let mut tasks = Vec::new();

        // BƯỚC 4.1: GOM DỮ LIỆU ĐẨY VÀO TASK
        for &id in chunk {
            if let Ok(Object::Stream(ref mut stream)) = doc.get_object_mut(id) {
                let filter = stream.dict.get(b"Filter").and_then(|o| o.as_name()).unwrap_or(b"");
                let filter_str = String::from_utf8_lossy(filter).into_owned();

                let width = stream.dict.get(b"Width").and_then(|o| o.as_i64()).unwrap_or(0) as u32;
                let height = stream.dict.get(b"Height").and_then(|o| o.as_i64()).unwrap_or(0) as u32;
                let color_space = String::from_utf8_lossy(
                    stream.dict.get(b"ColorSpace").and_then(|o| o.as_name()).unwrap_or(b"")
                ).into_owned();

                if filter_str == "DCTDecode" {
                    let _ = stream.decompress();
                    tasks.push(ImageTask { id, img_bytes: stream.content.clone(), filter: filter_str, width, height, color_space });
                } else if filter_str == "FlateDecode" {
                    let has_predictor = stream.dict.get(b"DecodeParms").and_then(|obj| obj.as_dict()).and_then(|dict| dict.get(b"Predictor")).is_ok();
                    if !has_predictor && width > 0 && height > 0 {
                        let _ = stream.decompress();
                        tasks.push(ImageTask { id, img_bytes: stream.content.clone(), filter: filter_str, width, height, color_space });
                    }
                }
            }
        }

        if tasks.is_empty() { continue; }

        // BƯỚC 4.2: XỬ LÝ SONG SONG TRÊN LUỒNG
        let processed_images: Vec<ProcessedImage> = pool.install(|| {
            tasks.into_par_iter().filter_map(|task| {
                let mut img_opt = None;

                if task.filter == "DCTDecode" {
                    img_opt = image::load_from_memory(&task.img_bytes).ok();
                } else if task.filter == "FlateDecode" {
                    let len = task.img_bytes.len();
                    let expected_rgb = (task.width * task.height * 3) as usize;
                    let expected_gray = (task.width * task.height) as usize;
                    let expected_rgba = (task.width * task.height * 4) as usize;

                    if len == expected_rgb {
                        if let Some(rgb) = RgbImage::from_raw(task.width, task.height, task.img_bytes.clone()) {
                            img_opt = Some(DynamicImage::ImageRgb8(rgb));
                        }
                    } else if len == expected_gray {
                        if let Some(gray) = GrayImage::from_raw(task.width, task.height, task.img_bytes.clone()) {
                            img_opt = Some(DynamicImage::ImageLuma8(gray));
                        }
                    } else if len == expected_rgba {
                        if let Some(rgba) = image::RgbaImage::from_raw(task.width, task.height, task.img_bytes.clone()) {
                            img_opt = Some(DynamicImage::ImageRgba8(rgba));
                        }
                    }
                }

                if let Some(mut img) = img_opt {
                    if grayscale {
                        img = DynamicImage::ImageLuma8(img.into_luma8());
                    }

                    let (w, h) = (img.width(), img.height());
                    if w > max_size || h > max_size {
                        img = img.resize(max_size, max_size, FilterType::Lanczos3);
                    }

                    let final_width = img.width();
                    let final_height = img.height();

                    let mut compressed_bytes = Vec::new();
                    let mut cursor = Cursor::new(&mut compressed_bytes);

                    let (is_gray_img, encode_result) = match img {
                        DynamicImage::ImageLuma8(gray) => {
                            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
                            (true, encoder.encode_image(&gray))
                        }
                        other => { 
                            let rgb = other.into_rgb8();
                            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
                            (false, encoder.encode_image(&rgb))
                        }
                    };

                    if encode_result.is_ok() {
                        return Some(ProcessedImage {
                            id: task.id,
                            compressed_bytes,
                            width: final_width,
                            height: final_height,
                            is_gray: is_gray_img, 
                        });
                    }
                }
                None
            }).collect()
        });

        // BƯỚC 4.3: LƯU LẠI VÀO DOC
        for p_img in processed_images {
            if let Ok(Object::Stream(ref mut stream)) = doc.get_object_mut(p_img.id) {
                stream.content = p_img.compressed_bytes;
                stream.dict.set("Width", Object::Integer(p_img.width as i64));
                stream.dict.set("Height", Object::Integer(p_img.height as i64));
                stream.dict.set("Filter", Object::Name(b"DCTDecode".to_vec())); 
                
                if p_img.is_gray {
                    stream.dict.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
                } else {
                    stream.dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
                }

                let _ = stream.compress();
            }
        }
    }

    if cancel_flag.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    // 5. LƯU PDF VỚI ĐIỀU KIỆN AN TOÀN
    {
        let mut file = fs::File::create(output_path).map_err(|e| format!("Error creating file: {}", e))?;
        let safe_to_use_obj_streams = false;

        let options = SaveOptions::builder()
            .use_object_streams(safe_to_use_obj_streams)        
            .use_xref_streams(safe_to_use_obj_streams)          
            .max_objects_per_stream(200)     
            .compression_level(9)            
            .build();

        doc.save_with_options(&mut file, options).map_err(|e| format!("Error saving file: {}", e))?;
    } 

    // 6. KIỂM TRA LẠI SIZE BẢO HIỂM
    let original_size = fs::metadata(input_path).map(|m| m.len()).unwrap_or(0);
    let new_size = fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);

    if new_size >= original_size && original_size > 0 {
        let _ = fs::remove_file(output_path);
        let _ = fs::copy(input_path, output_path);
    }

    Ok(())
}
