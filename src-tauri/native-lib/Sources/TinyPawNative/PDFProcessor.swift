import Foundation
import PDFKit
import AppKit
import Quartz

public class PDFProcessor {
    private static func imageToBase64(_ image: NSImage) -> String? {
        // Tạo một bitmap representation mới với kích thước của image
        guard let tiffData = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData) else {
            // Nếu tiffRepresentation fail (thường bị với icon hệ thống), ta dùng cách vẽ đè
            let size = image.size
            guard size.width > 0 && size.height > 0 else { return nil }
            
            let offscreenRep = NSBitmapImageRep(
                bitmapDataPlanes: nil,
                pixelsWide: Int(size.width),
                pixelsHigh: Int(size.height),
                bitsPerSample: 8,
                samplesPerPixel: 4,
                hasAlpha: true,
                isPlanar: false,
                colorSpaceName: .deviceRGB,
                bytesPerRow: 0,
                bitsPerPixel: 0
            )
            
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: offscreenRep!)
            image.draw(at: .zero, from: .zero, operation: .sourceOver, fraction: 1.0)
            NSGraphicsContext.restoreGraphicsState()
            
            guard let jpegData = offscreenRep?.representation(using: .jpeg, properties: [.compressionFactor: 0.8]) else {
                return nil
            }
            return "data:image/jpeg;base64,\(jpegData.base64EncodedString())"
        }
        
        // Nếu lấy được tiffData bình thường (cho file PDF thật)
        guard let jpegData = bitmap.representation(using: .jpeg, properties: [.compressionFactor: 0.8]) else {
            return nil
        }
        return "data:image/jpeg;base64,\(jpegData.base64EncodedString())"
    }
    
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
        
        // Thử lấy thumbnail từ trang đầu tiên
        if let document = PDFDocument(url: url), let page = document.page(at: 0) {
            if !document.isLocked {
                let pageRect = page.bounds(for: .mediaBox)
                let aspectRatio = pageRect.height / pageRect.width
                let targetHeight = targetWidth * aspectRatio
                let targetSize = NSSize(width: targetWidth, height: targetHeight)
                
                let thumbnail = page.thumbnail(of: targetSize, for: .mediaBox)
                if let base64 = imageToBase64(thumbnail) {
                    return base64
                }
            }
        }
        
        // --- FALLBACK: Nếu có pass hoặc lỗi, lấy Icon hệ thống ---
        // Lấy icon mặc định cho định dạng file .pdf
        return "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9IjYxNSIgdmlld0JveD0iMCAwIDUwMCA2MTUiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+CjxwYXRoIGQ9Ik0yNSAwLjVIMzU5Ljg4OUMzNjYuMzI5IDAuNSAzNzIuNTExIDMuMDM2MjMgMzc3LjA5NiA3LjU1OTU3TDQ5Mi4yMDcgMTIxLjEzMkM0OTYuODczIDEyNS43MzYgNDk5LjUgMTMyLjAxNyA0OTkuNSAxMzguNTcyVjU5MEM0OTkuNSA2MDMuNTMxIDQ4OC41MzEgNjE0LjUgNDc1IDYxNC41SDI1QzExLjQ2OSA2MTQuNSAwLjUgNjAzLjUzMSAwLjUgNTkwVjI1QzAuNTAwMDA3IDExLjY4MDYgMTEuMTI4NyAwLjg0MzQ1MiAyNC4zNjcyIDAuNTA3ODEyTDI1IDAuNVoiIGZpbGw9IiNGRjRCNDIiIHN0cm9rZT0iI0U0RTRFNCIvPgo8cGF0aCBkPSJNMCA0MjBINTAwVjU5MEM1MDAgNjAzLjgwNyA0ODguODA3IDYxNSA0NzUgNjE1SDI1QzExLjE5MjkgNjE1IDAgNjAzLjgwNyAwIDU5MFY0MjBaIiBmaWxsPSIjRTAxQzEyIi8+CjxwYXRoIGQ9Ik0yMTYuNjIyIDEwNy44OUMyMTguMDc0IDgxLjY1NjUgMjUwLjQ4NyA3OC40NjI5IDI1OC45NTIgMTA3Ljg5QzI2NS43MjUgMTMxLjQzMSAyNTQuODM0IDE2OS4yNTIgMjQ3LjkgMTk2LjI1OEMyNTQuODM0IDIxMi4xMzcgMjc3LjQ4NSAyNDEuNTY0IDI5MC45ODUgMjU0LjU2NkMzMzYuMDYgMjQ3LjcyMyAzNjEuNDU4IDI0OC40ODYgMzc4LjE2MiAyNTYuMzkxQzQwMC44MTQgMjY3LjExMyAzOTYuMDA5IDI5My40OSAzNzQuMDQzIDI5NS42MjZDMzU1LjI4MSAyOTcuNDUxIDMyNy41OTQgMjk1LjYyNiAyODguMjM5IDI2My4wMDZDMjc4LjAxOSAyNjQuMzc1IDI0Ny41NTcgMjcwLjI2IDIxMS4xMzEgMjgyLjg1MkMxOTcuMTc0IDMwNS4yMDcgMTc3LjcyNCAzMzUuNzgyIDE2MS4yNSAzNDkuMDA1QzEzNC4yNTEgMzcwLjY3NSAxMTMuODg3IDM2My42MDQgMTA4LjE2NyAzNTEuNzQyQzEwMC4zODcgMzMwLjc1NiAxMzEuOTYzIDMxMS4xMzggMTg5LjM5NCAyODIuODUyQzE5OC40NyAyNjQuOTA3IDIxOS40MTQgMjIwLjAzIDIzMC41NzkgMTg0LjA3OUMyMjUuNjk4IDE3Mi40NDUgMjE0Ljc5MiAxNDAuOTY2IDIxNi42MjIgMTA3Ljg5Wk0xODEuNjE1IDI5NS4xN0MxMDMuMzYyIDMyOC44MzUgMTA3LjAyMyAzNTEuNTE0IDEyMS42NjcgMzUzLjc5NUMxMzYuMzExIDM1Ni4wNzYgMTU0LjM4NiAzMzcuNTk5IDE4MS42MTUgMjk1LjE3Wk0zNzMuMTI4IDI3NS4wOTZDMzcyLjIxMyAyNTMuNjU0IDMzOS4yNjQgMjUzLjY1MyAyOTYuMjQ4IDI2MC45NTNDMzQ2LjM1NyAyOTYuNTM5IDM3My4xMjggMjkwLjE1MiAzNzMuMTI4IDI3NS4wOTZaTTI0NC4wOCAyMDQuODM3QzIzMS45OTggMjQwLjQyMyAyMjAuMjgzIDI2NS40NCAyMTUuOTM2IDI3My41QzI0OC41MTggMjYzLjQ2MyAyNzQuMzU5IDI1OC4wNjQgMjgzLjIwNiAyNTYuNjE5QzI2NS4wODQgMjM2LjE4IDI0OS41NzEgMjEzLjU4MSAyNDQuMDggMjA0LjgzN1pNMjM0LjY5OCAxNzIuNjczQzI1My42ODkgMTEyLjIyMyAyNDcuMjgzIDkzLjA2MiAyMzIuODY4IDk1LjExNDhDMjE3Ljc2NyA5Ny42MjQxIDIxOS44MjYgMTQwLjUwOSAyMzQuNjk4IDE3Mi42NzNaIiBmaWxsPSJ3aGl0ZSIvPgo8cGF0aCBkPSJNMTcyLjU5IDUyNi4yOTRIMTQ4Ljc4NlY1MTIuNTgzSDE3Mi41OUMxNzYuNDgzIDUxMi41ODMgMTc5LjYzNiA1MTEuOTQ4IDE4Mi4wNDggNTEwLjY3OUMxODQuNTAyIDUwOS4zNjcgMTg2LjMwMSA1MDcuNTkgMTg3LjQ0MyA1MDUuMzQ3QzE4OC41ODYgNTAzLjA2MiAxODkuMTU3IDUwMC40NTkgMTg5LjE1NyA0OTcuNTM5QzE4OS4xNTcgNDk0LjcwNCAxODguNTg2IDQ5Mi4wNTkgMTg3LjQ0MyA0ODkuNjA0QzE4Ni4zMDEgNDg3LjE1IDE4NC41MDIgNDg1LjE2MSAxODIuMDQ4IDQ4My42MzhDMTc5LjYzNiA0ODIuMTE0IDE3Ni40ODMgNDgxLjM1MyAxNzIuNTkgNDgxLjM1M0gxNTQuNDk5VjU2MEgxMzcuMDQzVjQ2Ny41NzhIMTcyLjU5QzE3OS43ODQgNDY3LjU3OCAxODUuOTIgNDY4Ljg2OSAxOTAuOTk4IDQ3MS40NUMxOTYuMTE4IDQ3My45ODkgMjAwLjAxMiA0NzcuNTIzIDIwMi42NzggNDgyLjA1MUMyMDUuMzg2IDQ4Ni41MzYgMjA2Ljc0IDQ5MS42NTcgMjA2Ljc0IDQ5Ny40MTJDMjA2Ljc0IDUwMy4zNzkgMjA1LjM4NiA1MDguNTIxIDIwMi42NzggNTEyLjgzN0MyMDAuMDEyIDUxNy4xNTMgMTk2LjExOCA1MjAuNDc1IDE5MC45OTggNTIyLjgwM0MxODUuOTIgNTI1LjEzIDE3OS43ODQgNTI2LjI5NCAxNzIuNTkgNTI2LjI5NFpNMjQ4LjA2MyA1NjBIMjI4LjEzMkwyMjguMjU5IDU0Ni4yODlIMjQ4LjA2M0MyNTMuNDM4IDU0Ni4yODkgMjU3Ljk0NSA1NDUuMTA0IDI2MS41ODQgNTQyLjczNEMyNjUuMjIzIDU0MC4zMjIgMjY3Ljk3NCA1MzYuODczIDI2OS44MzYgNTMyLjM4OEMyNzEuNjk4IDUyNy44NiAyNzIuNjI5IDUyMi40NDMgMjcyLjYyOSA1MTYuMTM4VjUxMS4zNzdDMjcyLjYyOSA1MDYuNTEgMjcyLjEgNTAyLjIxNSAyNzEuMDQyIDQ5OC40OTFDMjY5Ljk4NCA0OTQuNzY3IDI2OC40MTggNDkxLjYzNiAyNjYuMzQ1IDQ4OS4wOTdDMjY0LjMxMyA0ODYuNTU4IDI2MS43OTYgNDg0LjYzMiAyNTguNzkxIDQ4My4zMkMyNTUuNzg2IDQ4Mi4wMDggMjUyLjMzOCA0ODEuMzUzIDI0OC40NDQgNDgxLjM1M0gyMjcuNzUxVjQ2Ny41NzhIMjQ4LjQ0NEMyNTQuNjIzIDQ2Ny41NzggMjYwLjI1MSA0NjguNjE1IDI2NS4zMjkgNDcwLjY4OEMyNzAuNDUgNDcyLjc2MiAyNzQuODcyIDQ3NS43NDUgMjc4LjU5NiA0NzkuNjM5QzI4Mi4zNjIgNDgzLjQ5IDI4NS4yNCA0ODguMTAyIDI4Ny4yMjkgNDkzLjQ3N0MyODkuMjYgNDk4Ljg1MSAyOTAuMjc1IDUwNC44NiAyOTAuMjc1IDUxMS41MDRWNTE2LjEzOEMyOTAuMjc1IDUyMi43MzkgMjg5LjI2IDUyOC43NDggMjg3LjIyOSA1MzQuMTY1QzI4NS4yNCA1MzkuNTM5IDI4Mi4zNjIgNTQ0LjE1MiAyNzguNTk2IDU0OC4wMDNDMjc0Ljg3MiA1NTEuODU0IDI3MC40MjggNTU0LjgxNiAyNjUuMjY2IDU1Ni44OUMyNjAuMTAzIDU1OC45NjMgMjU0LjM2OSA1NjAgMjQ4LjA2MyA1NjBaTTIzNy45MDcgNDY3LjU3OFY1NjBIMjIwLjQ1MVY0NjcuNTc4SDIzNy45MDdaTTMyMi41ODUgNDY3LjU3OFY1NjBIMzA1LjEyOVY0NjcuNTc4SDMyMi41ODVaTTM1OS44NDYgNTA3LjUwNVY1MjEuMjc5SDMxOC4wMTVWNTA3LjUwNUgzNTkuODQ2Wk0zNjQuNzMzIDQ2Ny41NzhWNDgxLjM1M0gzMTguMDE1VjQ2Ny41NzhIMzY0LjczM1oiIGZpbGw9IndoaXRlIi8+Cjwvc3ZnPgo="
    }
}
