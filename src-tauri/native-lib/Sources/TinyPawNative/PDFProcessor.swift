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
}
