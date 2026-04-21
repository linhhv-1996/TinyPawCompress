import SwiftRs
import Foundation

// ---------------- XỬ LÝ PDF ----------------

@_cdecl("get_pdf_meta_swift")
public func getPdfMetaSwift(path: SRString) -> SRString {
    let inputPath = path.toString()
    let meta = PDFProcessor.getMetadata(from: inputPath)
    return SRString(meta)
}

@_cdecl("generate_pdf_thumbnail_swift")
public func generatePdfThumbnailSwift(path: SRString) -> SRString {
    let inputPath = path.toString()
    if let thumbBase64 = PDFProcessor.generateThumbnail(from: inputPath) {
        return SRString(thumbBase64)
    }
    return SRString("")
}

@_cdecl("compress_pdf_swift")
public func compressPdfSwift(args: SRString) -> SRString {
    let jsonArgs = args.toString()
    let result = PDFProcessor.compressPDF(jsonArgs: jsonArgs)
    return SRString(result)
}


// ---------------- XỬ LÝ ẢNH ----------------

@_cdecl("get_image_meta_swift")
public func getImageMetaSwift(path: SRString) -> SRString {
    let inputPath = path.toString()
    let meta = ImageProcessor.getMetadata(from: inputPath)
    return SRString(meta)
}

@_cdecl("generate_image_thumbnail_swift")
public func generateImageThumbnailSwift(path: SRString) -> SRString {
    let inputPath = path.toString()
    if let thumbBase64 = ImageProcessor.generateThumbnail(from: inputPath) {
        return SRString(thumbBase64)
    }
    // Lỗi thì trả về chuỗi rỗng
    return SRString("")
}

@_cdecl("compress_image_swift")
public func compressImageSwift(args: SRString) -> SRString {
    let jsonArgs = args.toString()
    // Hàm này mình sẽ viết ở bước 2
    let result = ImageProcessor.compressImage(jsonArgs: jsonArgs) 
    return SRString(result)
}

// ---------------- XỬ LÝ VIDEO ----------------

@_cdecl("get_video_meta_swift")
public func getVideoMetaSwift(path: SRString) -> SRString {
    let inputPath = path.toString()
    let meta = VideoProcessor.getMetadata(from: inputPath)
    return SRString(meta)
}

@_cdecl("generate_video_thumbnail_swift")
public func generateVideoThumbnailSwift(path: SRString) -> SRString {
    let inputPath = path.toString()
    if let thumbBase64 = VideoProcessor.generateThumbnail(from: inputPath) {
        return SRString(thumbBase64)
    }
    // Lỗi thì trả về chuỗi rỗng để Rust parse ra None
    return SRString("")
}

@_cdecl("compress_video_swift")
public func compressVideoSwift(args: SRString) -> SRString {
    let jsonArgs = args.toString()
    let result = VideoProcessor.compressVideo(jsonArgs: jsonArgs)
    return SRString(result)
}

@_cdecl("cancel_video_swift")
public func cancelVideoSwift(id: SRString) {
    let videoId = id.toString()
    VideoProcessor.cancelVideo(id: videoId)
}

