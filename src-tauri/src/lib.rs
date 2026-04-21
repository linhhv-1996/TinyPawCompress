use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
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

swift!(fn cancel_video_swift(id: &SRString));


mod utils;
mod pdf_compressor;
mod image_compressor;

// ==========================================
// 1. TẠO STRUCT LƯU TRỮ CẤU HÌNH (PRO GATE)
// ==========================================
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub is_pro: bool,
    pub processed_files_count: u32,
    pub license_key: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            is_pro: false,
            processed_files_count: 0,
            license_key: None,
        }
    }
}

// Hàm phụ trợ lấy đường dẫn file config ẩn của hệ điều hành
// macOS: ~/Library/Application Support/com.tinypaw.app/config.json
fn get_config_path(app_handle: &AppHandle) -> PathBuf {
    let app_dir = app_handle.path().app_data_dir().expect("Lỗi đọc thư mục AppData");
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).unwrap();
    }
    app_dir.join("config.json")
}

fn load_config(app_handle: &AppHandle) -> AppConfig {
    let path = get_config_path(app_handle);
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        AppConfig::default()
    }
}

fn save_config(app_handle: &AppHandle, config: &AppConfig) {
    let path = get_config_path(app_handle);
    if let Ok(content) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, content);
    }
}

struct AppState {
    cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    app_config: Mutex<AppConfig>,
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
async fn handle_dropped_files(app: AppHandle, paths: Vec<String>, state: State<'_, AppState>) -> Result<Vec<AppFile>, String> {
    let resource_path = app
        .path()
        .resolve("resources/libpdfium.dylib", BaseDirectory::Resource)
        .expect("failed to resolve resource");

    // 1. Đếm số lượng file hợp lệ thực tế từ mảng `paths` do user kéo vào
    let mut incoming_count = 0;
    for p in &paths {
        let path_obj = Path::new(p);
        if path_obj.is_file() {
            let ext = path_obj.extension().unwrap_or_default().to_string_lossy().to_lowercase();
            // Chỉ đếm những file app đang hỗ trợ
            if matches!(ext.as_str(), "mp4" | "mov" | "m4v" | "pdf" | "jpg" | "jpeg" | "png" | "webp") {
                incoming_count += 1;
            }
        }
    }

    // 2. Tiến hành check giới hạn Free/Pro
    {
        let mut config = state.app_config.lock().unwrap();
        if !config.is_pro {
            let free_limit = 5;

            if config.processed_files_count + incoming_count > free_limit {
                return Err("LIMIT_REACHED".to_string()); // Ném lỗi về Svelte
            }
        }
    }

    // 3. Xử lý thông tin file và tạo thumbnail (Giữ nguyên 100% code cũ của bạn)
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
            "mp4" | "mov" | "m4v" => "video",
            "pdf" => "pdf",
            "jpg" | "jpeg" | "png" | "webp" => "image",
            _ => continue,
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

    Ok(files) // Trả về danh sách ngay lập tức
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
    app: AppHandle,
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
        "id": id.clone(),
        "inputPath": input_path,
        "outputPath": output_path,
        "profile": profile,
        "targetSize": target_size,
        "codec": codec,
        "muteAudio": mute_audio,
    }).to_string();

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

        {
            let mut config = state.app_config.lock().unwrap();
            if !config.is_pro {
                config.processed_files_count += 1;
                save_config(&app, &config);
            }
        }

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


#[tauri::command]
async fn compress_pdf_command(
    id: String,
    input_path: String,
    output_path: String,
    profile: String,
    grayscale: bool,
    strip_meta: bool,
    unlock_pdf: bool, // THÊM NHẬN PARAM TỪ FRONTEND
    password: String, // THÊM NHẬN PARAM TỪ FRONTEND
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<CompressResult, String> {
    // 1. Setup cờ Cancel
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .cancel_flags
        .lock()
        .unwrap()
        .insert(id.clone(), cancel_flag.clone());

    let output_clone = output_path.clone();
    
    // THÊM: Clone password để move vào luồng ngầm (tránh lỗi borrow checker)
    let password_clone = password.clone(); 

    let cancel_flag_for_pdf = cancel_flag.clone();

    // 2. Chạy nén ngầm hoàn toàn bằng RUST (Bỏ Swift)
    let result = tauri::async_runtime::spawn_blocking(move || {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        // Gọi logic nén PDF từ file pdf_compressor.rs
        crate::pdf_compressor::compress_pdf(
            &input_path,
            &output_clone,
            &profile,
            grayscale,
            strip_meta,
            unlock_pdf,       // TRUYỀN XUỐNG
            &password_clone,  // TRUYỀN XUỐNG DƯỚI DẠNG &str
            cancel_flag_for_pdf,
        )
    })
    .await
    .map_err(|e| format!("Crash luồng nén PDF: {}", e))?;

    state.cancel_flags.lock().unwrap().remove(&id);

    // 3. Xử lý kết quả trả về
    match result {
        Ok(_) => {
            let meta = std::fs::metadata(&output_path).map_err(|e| format!("Error reading new file: {}", e))?;
            let new_size_text = crate::utils::format_size(meta.len());

            {
                let mut config = state.app_config.lock().unwrap();
                if !config.is_pro {
                    config.processed_files_count += 1;
                    save_config(&app, &config);
                }
            }
            
            Ok(CompressResult {
                id,
                success: true,
                new_size_bytes: meta.len(),
                new_size_text,
                error_msg: String::new(),
            })
        }
        Err(err_msg) => {
            Ok(CompressResult {
                id,
                success: false,
                new_size_bytes: 0,
                new_size_text: String::new(),
                error_msg: err_msg,
            })
        }
    }
}


#[tauri::command]
async fn compress_image_command(
    id: String,
    input_path: String,
    output_path: String,
    quality_value: u8,
    max_width: String,
    format: String,
    strip_exif: bool,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<CompressResult, String> {
    
    // 1. Setup cờ Cancel
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .cancel_flags
        .lock()
        .unwrap()
        .insert(id.clone(), cancel_flag.clone());

    let input_clone = input_path.clone();
    let output_clone = output_path.clone();
    let format_clone = format.clone();

    // 2. Chạy luồng ngầm xử lý nén qua libcaesium (Rust)
    let result = tauri::async_runtime::spawn_blocking(move || {
        // Kiểm tra xem User có bấm Cancel trong lúc chờ tới lượt không
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        crate::image_compressor::process_image(
            &input_clone,
            &output_clone,
            quality_value,
            &max_width,
            &format_clone,
            strip_exif,
        )
    })
    .await
    .map_err(|e| format!("Crash luồng nén Image: {}", e))?;

    // 3. Hoàn tất xử lý, dọn dẹp cờ Cancel
    state.cancel_flags.lock().unwrap().remove(&id);

    // 4. Trả kết quả
    match result {
        Ok(_) => {
            let meta = std::fs::metadata(&output_path).map_err(|e| format!("Lỗi đọc file ảnh mới: {}", e))?;
            let new_size_bytes = meta.len();
            let new_size_text = crate::utils::format_size(new_size_bytes);

            {
                let mut config = state.app_config.lock().unwrap();
                if !config.is_pro {
                    config.processed_files_count += 1;
                    save_config(&app, &config);
                }
            }
            
            Ok(CompressResult {
                id,
                success: true,
                new_size_bytes,
                new_size_text,
                error_msg: String::new(),
            })
        }
        Err(err_msg) => {
            Ok(CompressResult {
                id,
                success: false,
                new_size_bytes: 0,
                new_size_text: String::new(),
                error_msg: err_msg, // Chữ "Cancelled" sẽ được FE bắt và ẩn lỗi đỏ
            })
        }
    }
}


#[tauri::command]
fn cancel_compression_command(id: String, file_type: String, state: State<'_, AppState>) {
    // 1. Đặt cờ cho Rust (để dừng các task đang ở hàng đợi, chưa kịp gọi xuống Swift)
    if let Some(flag) = state.cancel_flags.lock().unwrap().get(&id) {
        flag.store(true, Ordering::Relaxed);
    }
    
    // 2. Chỉ can thiệp sâu xuống Swift nếu đó là Video
    if file_type == "video" {
        let sr_id = SRString::from(id.as_str());
        unsafe { cancel_video_swift(&sr_id) };
    }
    // Với PDF, quá trình write() của macOS chạy đồng bộ rất nhanh và không hỗ trợ ngắt giữa chừng,
    // nên ta không gọi cancel_pdf_swift, nó sẽ tự xong trong chốc lát và trả kết quả.
}


// ==========================================
// 3. API CHO FRONTEND LẤY THÔNG TIN & NHẬP KEY
// ==========================================
#[tauri::command]
fn get_pro_status(state: State<'_, AppState>) -> AppConfig {
    let config = state.app_config.lock().unwrap().clone();
    config
}

#[tauri::command]
async fn verify_license(key: String, app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    // TẠI ĐÂY BẠN CÓ THỂ GỌI API ĐẾN LEMONSQUEEZY / GUMROAD TRONG TƯƠNG LAI
    // Tạm thời mình hardcode key là "TINYPAW-PRO"
    let is_valid = key.trim() == "TINYPAW-PRO"; 

    if is_valid {
        let mut config = state.app_config.lock().unwrap();
        config.is_pro = true;
        config.license_key = Some(key);
        save_config(&app, &config);
        Ok(true)
    } else {
        Err("License key is invalid!".to_string())
    }
}

#[tauri::command]
fn check_compression_limit(count: u32, state: State<'_, AppState>) -> Result<(), String> {
    let config = state.app_config.lock().unwrap();
    if !config.is_pro {
        if config.processed_files_count + count > 5 {
            return Err("LIMIT_REACHED".to_string());
        }
    }
    Ok(())
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Load config từ ổ cứng lên RAM ngay khi bật app
            let config = load_config(app.handle());
            
            // Manage AppState ở đây thay vì bên dưới
            app.manage(AppState {
                cancel_flags: Mutex::new(HashMap::new()),
                app_config: Mutex::new(config),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            handle_dropped_files,
            compress_pdf_command,
            compress_video_command,
            compress_image_command,
            cancel_compression_command,
            check_compression_limit,
            get_pro_status,
            verify_license,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
