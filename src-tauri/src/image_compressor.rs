use caesium::{compress, convert, parameters::CSParameters, SupportedFileTypes};
use std::path::Path;
use std::fs;

pub fn process_image(
    input_path: &str,
    output_path: &str,
    quality: u8,
    max_width: &str,
    format: &str,
    strip_exif: bool,
) -> Result<(), String> {
    // 1. LẤY DUNG LƯỢNG FILE GỐC (Trước khi nén)
    let original_size = fs::metadata(input_path).map(|m| m.len()).unwrap_or(0);

    let mut parameters = CSParameters::new();
    parameters.keep_metadata = !strip_exif;
    
    parameters.jpeg.quality = quality as u32;
    parameters.webp.quality = quality as u32;
    parameters.png.quality = quality as u32;
    
    if let Ok(w) = max_width.parse::<u32>() {
        if w > 0 {
            parameters.width = w;
        }
    }

    let ext = Path::new(input_path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    let target_format = format.to_lowercase();
    let is_same_format = target_format == "original" ||
                         (target_format == "jpeg" && (ext == "jpg" || ext == "jpeg")) ||
                         (target_format == "webp" && ext == "webp") ||
                         (target_format == "png" && ext == "png");

    let result = if is_same_format {
        compress(input_path.to_string(), output_path.to_string(), &parameters)
    } else {
        match target_format.as_str() {
            "jpeg" | "jpg" => convert(input_path.to_string(), output_path.to_string(), &parameters, SupportedFileTypes::Jpeg),
            "webp" => convert(input_path.to_string(), output_path.to_string(), &parameters, SupportedFileTypes::WebP),
            "png" => convert(input_path.to_string(), output_path.to_string(), &parameters, SupportedFileTypes::Png),
            _ => compress(input_path.to_string(), output_path.to_string(), &parameters)
        }
    };

    // Nếu có lỗi từ quá trình nén thì ném ra ngay
    if result.is_err() {
        return result.map_err(|e| format!("Caesium lỗi: {:?}", e));
    }

    // 2. CHECK FALLBACK: Nếu nén thành công, check lại dung lượng
    let compressed_size = fs::metadata(output_path).map(|m| m.len()).unwrap_or(u64::MAX);
    
    if compressed_size > original_size {
        // Nếu nén xong mà to hơn (hoặc bằng) file gốc -> Chép file gốc đè sang thư mục output
        if let Err(e) = fs::copy(input_path, output_path) {
            return Err(format!("Lỗi khi copy file gốc làm fallback: {}", e));
        }
    }

    Ok(())
}
