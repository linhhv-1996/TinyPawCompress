// fn main() {
//     tauri_build::build()
// }
use swift_rs::SwiftLinker;

fn main() {
    // 1. Khai báo phiên bản macOS tối thiểu (phải khớp với Package.swift)
    // 10.15 là bản tối thiểu để chạy mượt các engine nén của Apple
    SwiftLinker::new("10.15")
        // 2. Tên Package: Phải đúng là "TinyPawNative" như trong Package.swift
        // 3. Đường dẫn: Trỏ vào folder chứa file Package.swift
        .with_package("TinyPawNative", "./native-lib")
        .link();

    // 4. Giữ lại dòng này để Tauri build các thành phần hệ thống khác
    tauri_build::build();
}
