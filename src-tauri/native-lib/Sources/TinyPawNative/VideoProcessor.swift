import Foundation
import AVFoundation
import AppKit

// Cấu trúc map chuẩn với JSON từ Rust
struct VideoCompressOptions: Codable {
    let id: String
    let inputPath: String
    let outputPath: String
    let profile: String     // "low", "balance", "high"
    let targetSize: Double  // Số MB người dùng nhập (nếu có)
    let codec: String       // "h264", "hevc"
    let muteAudio: Bool
}

public class VideoProcessor {

    nonisolated(unsafe) private static var activeSessions: [String: AVAssetExportSession] = [:]
    private static let sessionQueue = DispatchQueue(label: "com.tinypaw.sessions")

    // --- THÊM HÀM CANCEL ---
    public static func cancelVideo(id: String) {
        sessionQueue.sync {
            if let session = activeSessions[id] {
                session.cancelExport() // Bắn lệnh dừng ngay lập tức cho Apple
            }
        }
    }

    
    // ----------------------------------------------------
    // 1. HÀM LẤY METADATA (Không đổi)
    // ----------------------------------------------------
    public static func getMetadata(from path: String) -> String {
        let url = URL(fileURLWithPath: path)
        let asset = AVURLAsset(url: url)
        
        let duration = asset.duration
        var secs: Int64 = 0
        if duration.timescale != 0 {
            secs = Int64(duration.value) / Int64(duration.timescale)
        }
        let timeStr = String(format: "%02d:%02d", secs / 60, secs % 60)
        
        var resStr = "Video"
        let tracks = asset.tracks(withMediaType: .video)
        if let track = tracks.first {
            let size = track.naturalSize
            let height = Int(min(abs(size.width), abs(size.height))) // Lấy cạnh nhỏ nhất
            resStr = "\(height)p"
        }
        
        let ext = url.pathExtension.uppercased()
        return "\(resStr) • \(timeStr) • \(ext)"
    }
    
    // ----------------------------------------------------
    // 2. HÀM TẠO THUMBNAIL (Không đổi)
    // ----------------------------------------------------
    public static func generateThumbnail(from path: String) -> String? {
        let url = URL(fileURLWithPath: path)
        let asset = AVURLAsset(url: url)
        
        let duration = asset.duration
        var targetSeconds = 1.0
        
        if duration.timescale != 0 {
            let totalSecs = Double(duration.value) / Double(duration.timescale)
            targetSeconds = min(max(totalSecs * 0.1, 1.0), 5.0)
        }
        
        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        
        let time = CMTime(seconds: targetSeconds, preferredTimescale: 600)
        var actualTime = CMTime.zero
        
        do {
            let cgImage = try generator.copyCGImage(at: time, actualTime: &actualTime)
            let bitmapRep = NSBitmapImageRep(cgImage: cgImage)
            
            if let jpegData = bitmapRep.representation(using: .jpeg, properties: [:]) {
                let base64 = jpegData.base64EncodedString()
                return "data:image/jpeg;base64,\(base64)"
            }
        } catch {
            return nil
        }
        
        return nil
    }

    // ----------------------------------------------------
    // 3. HÀM NÉN VIDEO (SMART LOGIC)
    // ----------------------------------------------------
    public static func compressVideo(jsonArgs: String) -> String {
        guard let jsonData = jsonArgs.data(using: .utf8),
              let options = try? JSONDecoder().decode(VideoCompressOptions.self, from: jsonData) else {
            return "Error: Unable to read compression configuration."
        }
        
        let inputURL = URL(fileURLWithPath: options.inputPath)
        let outputURL = URL(fileURLWithPath: options.outputPath)
        
        // Xóa file đích nếu đã tồn tại
        if FileManager.default.fileExists(atPath: outputURL.path) {
            try? FileManager.default.removeItem(at: outputURL)
        }
        
        let asset = AVURLAsset(url: inputURL)
        var exportAsset: AVAsset = asset
        
        // --- BƯỚC A: Xử lý Tắt âm thanh ---
        if options.muteAudio {
            let composition = AVMutableComposition()
            guard let compositionVideoTrack = composition.addMutableTrack(withMediaType: .video, preferredTrackID: kCMPersistentTrackID_Invalid),
                  let sourceVideoTrack = asset.tracks(withMediaType: .video).first else {
                return "Error: Mute audio processing error."
            }
            do {
                try compositionVideoTrack.insertTimeRange(CMTimeRangeMake(start: .zero, duration: asset.duration), of: sourceVideoTrack, at: .zero)
                exportAsset = composition
            } catch {
                return "Error: Failed to delete audio"
            }
        }
        
        // --- BƯỚC B: SMART LOGIC CHỌN PRESET (CHỐNG UPSCALE) ---
        // Lấy độ phân giải ngắn nhất của video gốc (ví dụ 1920x1080 -> 1080)
        var sourceRes: CGFloat = 1080
        if let track = asset.tracks(withMediaType: .video).first {
            let size = track.naturalSize
            // abs để đề phòng số âm do Transform matrix, min để lấy cạnh ngắn (áp dụng cho cả video dọc/ngang)
            sourceRes = min(abs(size.width), abs(size.height))
        }
        
        var presetName = AVAssetExportPresetHighestQuality
        
        if options.codec == "hevc" {
            // HEVC: Apple hỗ trợ tối thiểu 1080p
            if options.profile == "low" || options.profile == "balance" {
                if sourceRes > 1080 {
                    presetName = AVAssetExportPresetHEVC1920x1080 // Ép khung hình 2K/4K xuống 1080p
                } else {
                    presetName = AVAssetExportPresetHEVCHighestQuality // Giữ nguyên độ phân giải gốc nhỏ hơn
                }
            } else {
                presetName = AVAssetExportPresetHEVCHighestQuality
            }
        } else {
            // H.264
            if options.profile == "low" && sourceRes > 720 {
                presetName = AVAssetExportPreset1280x720 // Ép xuống 720p nếu video quá to
            } else if options.profile == "balance" && sourceRes > 1080 {
                presetName = AVAssetExportPreset1920x1080 // Ép xuống 1080p nếu video 2K/4K
            } else {
                presetName = AVAssetExportPresetHighestQuality // Giữ nguyên gốc, tuyệt đối không upscale
            }
        }
        
        guard let exportSession = AVAssetExportSession(asset: exportAsset, presetName: presetName) else {
            return "Error: Device does not support this compression format."
        }
        
        exportSession.outputURL = outputURL
        exportSession.outputFileType = .mp4
        exportSession.shouldOptimizeForNetworkUse = true
        
        // --- BƯỚC C: ÉP DUNG LƯỢNG (GUARANTEE COMPRESSION) ---
        var finalTargetBytes: Int64 = 0
        
        if options.targetSize > 0 {
            // Trường hợp 1: Người dùng nhập tay số MB mong muốn
            finalTargetBytes = Int64(options.targetSize * 1024.0 * 1024.0)
        } else {
            // Trường hợp 2: Ép dung lượng tự động theo % của file gốc
            do {
                let attr = try FileManager.default.attributesOfItem(atPath: options.inputPath)
                if let fileSize = attr[.size] as? Int64 {
                    switch options.profile {
                    case "low":     finalTargetBytes = Int64(Double(fileSize) * 0.40) // Giảm 60%
                    case "balance": finalTargetBytes = Int64(Double(fileSize) * 0.70) // Giảm 30%
                    case "high":    finalTargetBytes = Int64(Double(fileSize) * 0.90) // Giảm 10%
                    default:        finalTargetBytes = Int64(Double(fileSize) * 0.90)
                    }
                }
            } catch {
                print("TinyPaw Warning: Không thể đọc dung lượng file gốc.")
            }
        }
        
        // Cấp lệnh ép dung lượng cho cỗ máy của Apple
        if finalTargetBytes > 0 {
            exportSession.fileLengthLimit = finalTargetBytes
        }

        sessionQueue.sync { activeSessions[options.id] = exportSession }
        
        // --- BƯỚC D: THỰC THI NÉN ---
        let semaphore = DispatchSemaphore(value: 0)
        exportSession.exportAsynchronously {
            semaphore.signal()
        }
        
        semaphore.wait() // Đợi quá trình nén chạy xong ngầm

        sessionQueue.sync { activeSessions.removeValue(forKey: options.id) }
        
        switch exportSession.status {
        case .completed:
            return "SUCCESS"
        case .failed:
            return "Error: \(exportSession.error?.localizedDescription ?? "Unknown")"
        case .cancelled:
            return "Cancelled" // SỬA THÀNH CHỮ "Cancelled" CHO KHỚP VỚI LOGIC CỦA SVELTE FE
        default:
            return "System error."
        }
    }
}
