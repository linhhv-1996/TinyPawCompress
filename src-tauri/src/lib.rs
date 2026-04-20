use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::State;
use tauri::{path::BaseDirectory, AppHandle, Emitter, Manager}; // Dùng Emitter để gửi event về UI
use uuid::Uuid;

use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

use crate::utils::format_size;

use swift_rs::{swift, SRString};

swift!(fn get_pdf_meta_swift(path: &SRString) -> SRString);
swift!(fn generate_pdf_thumbnail_swift(path: &SRString) -> SRString);
swift!(fn get_image_meta_swift(path: &SRString) -> SRString);
swift!(fn generate_image_thumbnail_swift(path: &SRString) -> SRString);
swift!(fn get_video_meta_swift(path: &SRString) -> SRString);
swift!(fn generate_video_thumbnail_swift(path: &SRString) -> SRString);
swift!(fn compress_video_swift(args: &SRString) -> SRString);


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

fn generate_image_thumbnail(path: &str) -> Option<String> {
    let sr_path = SRString::from(path);
    let result = unsafe { generate_image_thumbnail_swift(&sr_path) };
    let result_str = result.as_str();

    if result_str.is_empty() {
        None
    } else {
        Some(result_str.to_string())
    }
}

fn get_image_meta(path: &str) -> String {
    let sr_path = SRString::from(path);
    let result = unsafe { get_image_meta_swift(&sr_path) };
    result.as_str().to_string()
}

fn generate_pdf_thumbnail(path: &str) -> Option<String> {
    let sr_path = SRString::from(path);
    let result = unsafe { generate_pdf_thumbnail_swift(&sr_path) };
    let result_str = result.as_str();

    if result_str.is_empty() {
        None
    } else {
        Some(result_str.to_string())
    }
}

fn get_pdf_meta(path: &str) -> String {
    let sr_path = SRString::from(path);
    let result = unsafe { get_pdf_meta_swift(&sr_path) };
    result.as_str().to_string()
}

fn get_video_meta(path: &str) -> String {
    let sr_path = SRString::from(path);
    let result = unsafe { get_video_meta_swift(&sr_path) };
    result.as_str().to_string()
}

fn generate_video_thumbnail(path: &str) -> Option<String> {
    let sr_path = SRString::from(path);
    let result = unsafe { generate_video_thumbnail_swift(&sr_path) };
    let result_str = result.as_str();

    if result_str.is_empty() {
        None
    } else {
        Some(result_str.to_string())
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
            "pdf" => get_pdf_meta(&p),
            "video" => get_video_meta(&p),
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

        // tokio::spawn(async move {
        tauri::async_runtime::spawn_blocking(move || {
            let thumb_data = match f_type_clone.as_str() {
                "image" => generate_image_thumbnail(&p_clone),
                "pdf" => generate_pdf_thumbnail(&p_clone),
                "video" => generate_video_thumbnail(&p_clone),
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


#[tauri::command]
async fn compress_video_command(
    id: String,
    input_path: String,
    output_path: String,
    profile: String,
    target_size: f64,
    codec: String,
    mute_audio: bool,
    state: tauri::State<'_, AppState>,
) -> Result<CompressResult, String> {
    
    // 1. Setup cờ Cancel
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .cancel_flags
        .lock()
        .unwrap()
        .insert(id.clone(), cancel_flag.clone());

    // 2. Gom tham số thành JSON (Đã sửa lại key cho khớp)
    let json_args = serde_json::json!({
        "inputPath": input_path,
        "outputPath": output_path,
        "profile": profile,
        "targetSize": target_size,
        "codec": codec,
        "muteAudio": mute_audio,
    }).to_string();

    let output_clone = output_path.clone();

    // 3. Chạy ngầm trong spawn_blocking
    let result = tauri::async_runtime::spawn_blocking(move || {
        if cancel_flag.load(Ordering::Relaxed) {
            return "Cancelled".to_string();
        }

        let sr_args = SRString::from(json_args.as_str());
        let swift_res = unsafe { compress_video_swift(&sr_args) };
        swift_res.as_str().to_string()
    })
    .await
    .map_err(|e| format!("Crash luồng nén Video: {}", e))?;

    state.cancel_flags.lock().unwrap().remove(&id);

    // 4. Xử lý kết quả trả về
    if result == "SUCCESS" {
        let meta = fs::metadata(&output_path).map_err(|e| format!("Lỗi đọc file Video mới: {}", e))?;
        let new_size_text = crate::utils::format_size(meta.len());

        Ok(CompressResult {
            id,
            success: true,
            new_size_bytes: meta.len(),
            new_size_text,
            error_msg: String::new(),
        })
    } else {
        Ok(CompressResult {
            id,
            success: false,
            new_size_bytes: 0,
            new_size_text: String::new(),
            error_msg: result, 
        })
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
            compress_video_command,
            cancel_compression_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
