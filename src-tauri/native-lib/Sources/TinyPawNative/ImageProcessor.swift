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

    public static func compressImage(jsonArgs: String) -> String {
        guard let data = jsonArgs.data(using: .utf8),
              let args = try? JSONSerialization.jsonObject(with: data, options: []) as? [String: Any],
              let inputPath = args["inputPath"] as? String,
              let outputPath = args["outputPath"] as? String else {
            return "Lỗi đọc dữ liệu truyền vào"
        }
        
        // Ép sang NSNumber để ImageIO đọc được đúng tham số Quality
        let qualityValue = (args["qualityValue"] as? NSNumber)?.doubleValue ?? 80.0
        let format = args["format"] as? String ?? "original"
        let stripExif = args["stripExif"] as? Bool ?? true
        
        var maxWidth: CGFloat? = nil
        if let mwStr = args["maxWidth"] as? String, let mw = Double(mwStr), mw > 0 {
            maxWidth = CGFloat(mw)
        }
        
        let inputURL = URL(fileURLWithPath: inputPath)
        let outputURL = URL(fileURLWithPath: outputPath)
        
        guard let imageSource = CGImageSourceCreateWithURL(inputURL as CFURL, nil) else {
            return "Không thể mở file ảnh gốc"
        }
        
        // 1. XỬ LÝ LỖI WEBP BẰNG UNIFORM TYPE IDENTIFIERS
        let originalUTI = CGImageSourceGetType(imageSource) ?? ("public.jpeg" as CFString)
        var outputUTI = originalUTI
        
        if format == "jpeg" {
            outputUTI = "public.jpeg" as CFString
        } else if format == "webp" {
            outputUTI = "org.webmproject.webp" as CFString
        }
        
        guard let properties = CGImageSourceCopyPropertiesAtIndex(imageSource, 0, nil) as? [CFString: Any] else {
            return "Không thể đọc thông số ảnh"
        }
        
        let originalWidth = properties[kCGImagePropertyPixelWidth] as? CGFloat ?? 0
        let originalHeight = properties[kCGImagePropertyPixelHeight] as? CGFloat ?? 0
        
        // 2. TẠO DECODE OPTIONS ĐỂ GỠ EXIF XOAY
        var decodeOptions: [CFString: Any] = [
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceCreateThumbnailFromImageAlways: true
        ]
        
        if let maxW = maxWidth, maxW < originalWidth {
            decodeOptions[kCGImageSourceThumbnailMaxPixelSize] = maxW
        } else {
            let maxDimension = max(originalWidth, originalHeight)
            decodeOptions[kCGImageSourceThumbnailMaxPixelSize] = maxDimension > 0 ? maxDimension : 8000
        }
        
        guard let cgImage = CGImageSourceCreateThumbnailAtIndex(imageSource, 0, decodeOptions as CFDictionary) else {
            return "Lỗi render xử lý điểm ảnh"
        }
        
        // 3. KHỞI TẠO FILE DESTINATION
        guard let destination = CGImageDestinationCreateWithURL(outputURL as CFURL, outputUTI, 1, nil) else {
            if format == "webp" { return "Lỗi: Phiên bản macOS này không hỗ trợ lưu WebP" }
            return "Không thể khởi tạo file xuất"
        }
        
        // Cấu hình Nén Lossy BẮT BUỘC dùng NSNumber
        var writeOptions: [CFString: Any] = [
            kCGImageDestinationLossyCompressionQuality: NSNumber(value: qualityValue / 100.0)
        ]
        
        if !stripExif {
            var newProperties = properties
            newProperties[kCGImagePropertyOrientation] = nil
            if var tiff = newProperties[kCGImagePropertyTIFFDictionary] as? [CFString: Any] {
                tiff[kCGImagePropertyTIFFOrientation] = nil
                newProperties[kCGImagePropertyTIFFDictionary] = tiff
            }
            for (key, value) in newProperties {
                writeOptions[key] = value
            }
        }
        
        CGImageDestinationAddImage(destination, cgImage, writeOptions as CFDictionary)
        
        if !CGImageDestinationFinalize(destination) {
            return "Lỗi trong quá trình ghi dữ liệu ra ổ cứng"
        }
        
        // ---------------------------------------------------------
        // 4. LỚP BẢO HIỂM CHỐNG TĂNG SIZE (Đỉnh cao là ở đây)
        // ---------------------------------------------------------
        let fm = FileManager.default
        let originalSize = (try? fm.attributesOfItem(atPath: inputPath)[.size] as? NSNumber)?.intValue ?? 0
        let compressedSize = (try? fm.attributesOfItem(atPath: outputPath)[.size] as? NSNumber)?.intValue ?? 0
        
        if compressedSize >= originalSize && originalSize > 0 {
            try? fm.removeItem(at: outputURL)
            try? fm.copyItem(at: inputURL, to: outputURL)
            return "SKIPPED_BIGGER" // Gửi tín hiệu về cho Rust
        }
        
        return "SUCCESS"
    }
}
