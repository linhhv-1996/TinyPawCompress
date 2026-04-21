import Foundation
import AppKit
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

public class ImageProcessor {
    
    // Lấy thông tin kích thước và đuôi file
    public static func getMetadata(from path: String) -> String {
        let url = URL(fileURLWithPath: path)
        let ext = url.pathExtension.uppercased()
        
        // Dùng CGImageSource để đọc meta mà không cần load nguyên cái ảnh vào RAM
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any] else {
            return "Image • \(ext)"
        }
        
        let width = properties[kCGImagePropertyPixelWidth] as? Int ?? 0
        let height = properties[kCGImagePropertyPixelHeight] as? Int ?? 0
        
        if width > 0 && height > 0 {
            return "\(width)x\(height) • \(ext)"
        }
        
        return "Image • \(ext)"
    }
    
    // Tạo Thumbnail Base64 siêu tốc
    public static func generateThumbnail(from path: String, maxSize: CGFloat = 300) -> String? {
        let url = URL(fileURLWithPath: path)
        
        // Cấu hình để ImageIO tự động scale ảnh xuống ngay trong lúc đọc file
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageIfAbsent: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxSize
        ]
        
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let cgImage = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
            return nil
        }
        
        // Chuyển CGImage thành JPEG Base64
        let bitmapRep = NSBitmapImageRep(cgImage: cgImage)
        guard let jpegData = bitmapRep.representation(using: .jpeg, properties: [.compressionFactor: 0.8]) else {
            return nil
        }
        
        return "data:image/jpeg;base64,\(jpegData.base64EncodedString())"
    }
}
