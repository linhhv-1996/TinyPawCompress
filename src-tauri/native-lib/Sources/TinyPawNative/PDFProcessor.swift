import Foundation
import PDFKit
import AppKit
import Quartz

public class PDFProcessor {
    
    // Lấy số trang PDF
    public static func getMetadata(from path: String) -> String {
        let url = URL(fileURLWithPath: path)
        guard let document = PDFDocument(url: url) else {
            return "PDF Document"
        }
        return "\(document.pageCount) pages • PDF"
    }
    
    // Tạo Thumbnail Base64
    public static func generateThumbnail(from path: String, targetWidth: CGFloat = 300) -> String? {
        let url = URL(fileURLWithPath: path)
        guard let document = PDFDocument(url: url),
              let page = document.page(at: 0) else {
            return nil
        }
        
        // Tính toán kích thước thumbnail giữ nguyên tỷ lệ
        let pageRect = page.bounds(for: .mediaBox)
        let aspectRatio = pageRect.height / pageRect.width
        let targetHeight = targetWidth * aspectRatio
        let targetSize = NSSize(width: targetWidth, height: targetHeight)
        
        // Render thumbnail
        let image = page.thumbnail(of: targetSize, for: .mediaBox)
        
        // Chuyển sang JPEG -> Base64
        guard let tiffData = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData),
              let jpegData = bitmap.representation(using: .jpeg, properties: [.compressionFactor: 0.8]) else {
            return nil
        }
        
        let base64String = jpegData.base64EncodedString()
        return "data:image/jpeg;base64,\(base64String)"
    }

    public static func compressPDF(jsonArgs: String) -> String {
        guard let data = jsonArgs.data(using: .utf8),
              let args = try? JSONSerialization.jsonObject(with: data, options: []) as? [String: Any],
              let inputPath = args["inputPath"] as? String,
              let outputPath = args["outputPath"] as? String else {
            return "Invalid arguments format"
        }
        
        let profile = args["profile"] as? String ?? "ebook"
        let grayscale = args["grayscale"] as? Bool ?? false
        let stripMeta = args["stripMeta"] as? Bool ?? false
        
        let inputURL = URL(fileURLWithPath: inputPath)
        let outputURL = URL(fileURLWithPath: outputPath)
        
        guard let pdfDoc = PDFDocument(url: inputURL) else {
            return "Failed to open PDF document"
        }
        
        // 1. Tinh chỉnh lại Hệ số nén (Ép rát hơn) và THÊM ImageSizeMax
        var q: CGFloat = 0.5
        var targetDPI: Int = 150
        var imageSizeMax: Int = 1280 // Chiều dài tối đa của ảnh
        
        switch profile {
        case "screen": // Nén mạnh nhất cho màn hình/web
            q = 0.15        // Ép chất lượng JPEG xuống rất thấp
            targetDPI = 72
            imageSizeMax = 800
        case "ebook": // Cân bằng
            q = 0.4
            targetDPI = 144
            imageSizeMax = 1280
        case "printer": // Ít nén để in ấn
            q = 0.75
            targetDPI = 300
            imageSizeMax = 2048
        default:
            q = 0.5
            targetDPI = 144
            imageSizeMax = 1280
        }
        
        if stripMeta {
            pdfDoc.documentAttributes = [:]
        }
        
        // 2. Cấu hình Filter Dictionary CHUẨN TRỊ BỆNH TĂNG SIZE
        var colorSettings: [String: Any] = [
            "ImageSettings": [
                "Compression Quality": q,
                "ImageCompression": "ImageJPEGCompress",
                "ImageScaleSettings": [
                    "ImageResolution": targetDPI,
                    "ImageSizeMax": imageSizeMax // <-- VŨ KHÍ TỐI THƯỢNG ĐỂ GIẢM SIZE
                ]
            ]
        ]
        
        if grayscale {
            colorSettings["DocumentColorSpace"] = "Generic Gray"
            colorSettings["IntermediateColorSpace"] = "Generic Gray"
        }
        
        let filterDict: [String: Any] = [
            "Domains": ["Applications": true, "Printing": true],
            "FilterType": 1,
            "Name": "TinyPawDynamicCompressor",
            "FilterData": [
                "ColorSettings": colorSettings
            ]
        ]
        
        let tempFilterURL = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".qfilter")
        defer { try? FileManager.default.removeItem(at: tempFilterURL) }
        
        do {
            let plistData = try PropertyListSerialization.data(fromPropertyList: filterDict, format: .xml, options: 0)
            try plistData.write(to: tempFilterURL)
            
            guard let filter = QuartzFilter(url: tempFilterURL) else {
                return "Failed to initialize QuartzFilter"
            }
            
            // 3. Tiến hành xuất file
            let options = [PDFDocumentWriteOption(rawValue: "QuartzFilter"): filter]
            let success = pdfDoc.write(to: outputURL, withOptions: options)
            
            if !success { return "Failed to write compressed PDF" }
            
            // ---------------------------------------------------------
            // 4. LỚP BẢO HIỂM CHỐNG TĂNG SIZE KHÓ CHỊU CỦA MACOS
            // ---------------------------------------------------------
            let fm = FileManager.default
            let originalSize = (try? fm.attributesOfItem(atPath: inputPath)[.size] as? NSNumber)?.intValue ?? 0
            let compressedSize = (try? fm.attributesOfItem(atPath: outputPath)[.size] as? NSNumber)?.intValue ?? 0
            
            // Nếu file nén xong mà lại nặng hơn hoặc bằng file gốc -> Do PDF toàn text/vector hoặc Quartz dở chứng.
            // Xử lý: Xoá file lỗi đi, copy y nguyên file gốc sang output để bypass!
            if compressedSize >= originalSize && originalSize > 0 {
                print("⚠️ File nén bị to hơn gốc! Hoàn trả file gốc.")
                try? fm.removeItem(at: outputURL)
                try? fm.copyItem(at: inputURL, to: outputURL)
            }
            
            return "SUCCESS"
        } catch {
            return "Error applying filter: \(error.localizedDescription)"
        }
    }
}
