use base64::{engine::general_purpose, Engine as _};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::State;
use tauri::{path::BaseDirectory, AppHandle, Emitter, Manager}; // Dùng Emitter để gửi event về UI
use uuid::Uuid;

use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

use crate::utils::format_size;

mod pdf_compressor;
mod utils;

struct AppState {
    cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppFile {
    id: String,
    path: String,
    name: String,
    file_type: String,
    size_bytes: u64,
    size_text: String,
    thumbnail: Option<String>,
    metadata: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ThumbnailPayload {
    id: String,
    data: String,
}

// Struct trả về cho Svelte
#[derive(Serialize)]
pub struct CompressResult {
    pub id: String,
    pub success: bool,
    pub new_size_bytes: u64,
    pub new_size_text: String,
    pub error_msg: String,
}

// Hàm nén ảnh về thumbnail nhỏ để tối ưu RAM
fn generate_image_thumbnail(path: &str) -> Option<String> {
    let img = image::open(path).ok()?;
    let thumb = img.thumbnail(300, 300); // Resize về 300px
    let mut image_data: Vec<u8> = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut image_data), image::ImageFormat::Jpeg)
        .ok()?;
    Some(format!(
        "data:image/jpeg;base64,{}",
        general_purpose::STANDARD.encode(image_data)
    ))
}

fn generate_pdf_thumbnail(path: &str, lib_path: &std::path::PathBuf) -> Option<String> {
    let pdfium_bind = Pdfium::bind_to_library(lib_path);

    if let Err(e) = &pdfium_bind {
        println!("❌ Lỗi PDFium Binary: {:?}", e); // Xem lỗi ở terminal Tauri
        return None;
    }

    let pdfium = Pdfium::new(pdfium_bind.unwrap());

    // 2. Load file PDF
    let document = pdfium.load_pdf_from_file(path, None);
    if let Err(e) = &document {
        println!("❌ Không load được PDF: {:?}", e);
        return None;
    }

    let doc = document.unwrap();
    let first_page = doc.pages().get(0).ok()?;

    let render_config = PdfRenderConfig::new().set_target_width(300);
    let bitmap = first_page.render_with_config(&render_config).ok()?;

    let img = bitmap.as_image();

    let mut image_data: Vec<u8> = Vec::new();
    // Đảm bảo image format Jpeg đã được bật trong Cargo.toml
    if let Err(e) = img.write_to(&mut Cursor::new(&mut image_data), image::ImageFormat::Jpeg) {
        println!("❌ Lỗi encode ảnh: {:?}", e);
        return None;
    }

    Some(format!(
        "data:image/jpeg;base64,{}",
        general_purpose::STANDARD.encode(image_data)
    ))
}

// Hàm lấy thông tin Image
fn get_image_meta(path: &str) -> String {
    if let Ok(dim) = image::image_dimensions(path) {
        let ext = Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_uppercase();
        return format!("{}x{} • {}", dim.0, dim.1, ext);
    }
    "Image".to_string()
}

// Hàm lấy thông tin PDF (Dùng luôn Pdfium bạn đang có)
fn get_pdf_meta(path: &str, lib_path: &std::path::PathBuf) -> String {
    let pdfium_bind = Pdfium::bind_to_library(lib_path);
    if let Ok(bind) = pdfium_bind {
        let pdfium = Pdfium::new(bind);

        // Fix: Lưu kết quả vào một biến String sở hữu riêng (Owned String)
        let result = if let Ok(doc) = pdfium.load_pdf_from_file(path, None) {
            format!("{} pages • PDF", doc.pages().len())
        } else {
            "PDF Document".to_string()
        };

        return result; // Lúc này doc và pdfium có thể chết thoải mái, result vẫn sống
    }
    "PDF Document".to_string()
}

#[cfg(target_os = "macos")]
mod macos_vid {
    use super::*;
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, rc::Retained, ClassType};
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
    use objc2_av_foundation::{AVAssetImageGenerator, AVMediaTypeVideo, AVURLAsset};
    use objc2_core_media::CMTime;
    use objc2_foundation::NSSize;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString, NSURL};
    use std::ptr;

    pub fn generate_video_thumbnail(path: &str) -> Option<String> {
        unsafe {
            let ns_path = NSString::from_str(path);
            let url = NSURL::fileURLWithPath(&ns_path);
            let asset = AVURLAsset::assetWithURL(&url);

            // Lấy duration của video
            let duration: CMTime = msg_send![&asset, duration];

            // Tính toán thời điểm lấy thumbnail: 10% của video
            // Nếu video dài, lấy ở giây thứ 2-3 cho chắc cú.
            // CMTime: value / timescale = seconds.
            let target_seconds = if duration.timescale != 0 {
                let total_secs = duration.value as f64 / duration.timescale as f64;
                (total_secs * 0.1).min(5.0).max(1.0) // Lấy 10%, nhưng không quá 5s và ít nhất 1s
            } else {
                1.0
            };

            let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
            let _: () = msg_send![&*generator, setAppliesPreferredTrackTransform: true];

            // Thiết lập sai số (tolerance) bằng 0 để lấy chính xác frame đó nếu cần,
            // hoặc để mặc định (không set) để máy chạy nhanh hơn (nó sẽ lấy keyframe gần nhất).
            // Thường thumbnail thì không cần chính xác từng miligiây, nên để mặc định là tốt nhất.

            let time = CMTime::new((target_seconds * 600.0) as i64, 600);
            let mut actual_time = CMTime::new(0, 1);
            let mut error: *mut objc2_foundation::NSError = ptr::null_mut();

            let image_ref_ptr: *mut std::ffi::c_void = msg_send![
                &*generator,
                copyCGImageAtTime: time,
                actualTime: &mut actual_time,
                error: &mut error
            ];

            if image_ref_ptr.is_null() {
                return None;
            }

            let alloc = msg_send![NSBitmapImageRep::class(), alloc];
            let bitmap_rep: Option<Retained<NSBitmapImageRep>> = msg_send![
                alloc,
                initWithCGImage: image_ref_ptr
            ];

            let bitmap_rep = bitmap_rep?;
            let props = NSDictionary::<AnyObject, AnyObject>::new();

            let data: Option<Retained<NSData>> = msg_send![
                &*bitmap_rep,
                representationUsingType: NSBitmapImageFileType::JPEG.0,
                properties: &*props
            ];

            let data = data?;
            let bytes_ptr: *const u8 = msg_send![&*data, bytes];
            let length: usize = msg_send![&*data, length];
            let slice = std::slice::from_raw_parts(bytes_ptr, length);

            Some(format!(
                "data:image/jpeg;base64,{}",
                general_purpose::STANDARD.encode(slice)
            ))
        }
    }

    pub fn get_video_meta(path: &str) -> String {
        unsafe {
            let ns_path = NSString::from_str(path);
            let url = NSURL::fileURLWithPath(&ns_path);
            let asset = AVURLAsset::assetWithURL(&url);

            // 1. Lấy duration qua msg_send!
            let duration: CMTime = msg_send![&asset, duration];
            let secs = if duration.timescale != 0 {
                duration.value / duration.timescale as i64
            } else {
                0
            };
            let time_str = format!("{:02}:{:02}", secs / 60, secs % 60);

            // 2. Lấy tracks và resolution
            let tracks: Retained<NSArray<AnyObject>> =
                msg_send![&asset, tracksWithMediaType: AVMediaTypeVideo];
            let mut res_str = "Video".to_string();

            if tracks.count() > 0 {
                let track: *mut AnyObject = msg_send![&tracks, firstObject];
                // Dùng NSSize thay vì CGSize
                let size: NSSize = msg_send![track, naturalSize];
                let height = size.height as i32;
                res_str = format!("{}p", height);
            }

            let ext = Path::new(path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_uppercase();
            format!("{} • {} • {}", res_str, time_str, ext)
        }
    }
}

#[tauri::command]
async fn handle_dropped_files(app: AppHandle, paths: Vec<String>) -> Vec<AppFile> {
    let resource_path = app
        .path()
        .resolve("resources/libpdfium.dylib", BaseDirectory::Resource)
        .expect("failed to resolve resource");

    let mut files = Vec::new();

    for p in paths {
        let path_obj = Path::new(&p);
        if !path_obj.is_file() {
            continue;
        }

        let name = path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let ext = path_obj
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        // ext bây giờ là String, dùng &ext là đủ, không cần .as_str()
        let f_type = match ext.as_ref() {
            "mp4" | "mkv" | "mov" | "avi" => "video",
            "pdf" => "pdf",
            "jpg" | "jpeg" | "png" | "webp" => "image",
            _ => "other",
        };

        let metadata = fs::metadata(&p).ok();

        let size_in_bytes = metadata.map(|m| m.len()).unwrap_or(0);

        let id = Uuid::new_v4().to_string();

        let meta = match f_type {
            "image" => get_image_meta(&p),
            "pdf" => get_pdf_meta(&p, &resource_path),
            "video" => {
                #[cfg(target_os = "macos")]
                {
                    macos_vid::get_video_meta(&p)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    "Video".to_string()
                }
            }
            _ => "Other".to_string(),
        };

        files.push(AppFile {
            id: id.clone(),
            path: p.clone(),
            name,
            file_type: f_type.to_string(),
            size_bytes: size_in_bytes,
            size_text: format_size(size_in_bytes),
            thumbnail: None,
            metadata: meta,
        });

        // SPAWN BACKGROUND TASK: Không đợi, chạy ngầm để tạo thumb
        let app_clone = app.clone();
        let p_clone = p.clone();
        let f_type_clone = f_type.to_string();
        let lib_path_clone = resource_path.clone();

        // tokio::spawn(async move {
        tauri::async_runtime::spawn_blocking(move || {
            let thumb_data = match f_type_clone.as_str() {
                "image" => generate_image_thumbnail(&p_clone),
                "pdf" => generate_pdf_thumbnail(&p_clone, &lib_path_clone),
                "video" => {
                    #[cfg(target_os = "macos")]
                    {
                        macos_vid::generate_video_thumbnail(&p_clone)
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        None
                    } // Có thể bổ sung FFmpeg cho Windows/Linux sau
                }
                _ => None,
            };

            if let Some(data) = thumb_data {
                // Gửi event về cho Svelte khi thumb đã sẵn sàng
                // Dùng .ok() thay cho .unwrap() để app không bị crash (panic) nếu lỡ FE đóng giữa chừng
                app_clone.emit("thumbnail-ready", ThumbnailPayload { id, data }).ok();
            }
        });
    }

    files // Trả về danh sách ngay lập tức
}

#[tauri::command]
async fn compress_pdf_command(
    id: String,
    input_path: String,
    output_path: String,
    profile: String,
    grayscale: bool,
    strip_meta: bool,
    state: tauri::State<'_, AppState>,
) -> Result<CompressResult, String> {
    // 1. Setup cờ Cancel
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .cancel_flags
        .lock()
        .unwrap()
        .insert(id.clone(), cancel_flag.clone());

    let options = crate::pdf_compressor::PdfCompressOptions {
        profile,
        grayscale,
        strip_meta,
    };
    let output_clone = output_path.clone();

    // 2. Chạy nén ngầm
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::pdf_compressor::compress_pdf(&input_path, &output_clone, options, cancel_flag)
    })
    .await
    .map_err(|e| format!("Crash luồng nén: {}", e))?; // Lỗi panic của thread

    state.cancel_flags.lock().unwrap().remove(&id);

    // 3. Xử lý kết quả trả về
    match result {
        Ok(_) => {
            // Lấy size file mới nén xong
            let meta =
                fs::metadata(&output_path).map_err(|e| format!("Lỗi đọc file mới: {}", e))?;
            let new_size_text = crate::utils::format_size(meta.len()); // Dùng lại hàm format_size của mày

            Ok(CompressResult {
                id,
                success: true,
                new_size_bytes: meta.len(),
                new_size_text,
                error_msg: String::new(),
            })
        }
        Err(e) => {
            // Vẫn trả về Ok(CompressResult) để FE dễ hứng JSON, nhưng success = false
            Ok(CompressResult {
                id,
                success: false,
                new_size_bytes: 0,
                new_size_text: String::new(),
                error_msg: e,
            })
        }
    }
}

// Command này để FE gọi khi user bấm nút [X] Cancel
#[tauri::command]
fn cancel_compression_command(id: String, state: State<'_, AppState>) {
    if let Some(flag) = state.cancel_flags.lock().unwrap().get(&id) {
        flag.store(true, Ordering::Relaxed);
    }
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            cancel_flags: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            handle_dropped_files,
            compress_pdf_command,
            cancel_compression_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
