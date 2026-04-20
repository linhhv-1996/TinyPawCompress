// Định nghĩa Typescript Interfaces cho cấu hình từng loại file
export interface VideoSettings {
    qualityTab: string;
    qualityValue: number;
    targetSize: string | number;
    resolution: string;
    codec: string;
    muteAudio: boolean;
}

export interface PdfSettings {
    profile: string;
    grayscale: boolean;
    stripMeta: boolean;
    passwordProtect: boolean;
    password?: string;
}

export interface ImageSettings {
    qualityTab: string;
    qualityValue: number;
    maxWidth: string | number;
    format: string;
    stripExif: boolean;
}

// Hàm khởi tạo cài đặt mặc định dựa vào loại file
export function getDefaultSettings(fileType: string) {
    switch (fileType) {
        case 'video':
            return {
                qualityTab: 'balance',
                qualityValue: 80,
                targetSize: '',
                resolution: 'original',
                codec: 'h264',
                muteAudio: false
            } as VideoSettings;
            
        case 'pdf':
            return {
                profile: 'screen',
                grayscale: false,
                stripMeta: true,
                passwordProtect: false,
                password: ''
            } as PdfSettings;
            
        case 'image':
            return {
                qualityTab: 'balance',
                qualityValue: 80,
                maxWidth: '',
                format: 'original',
                stripExif: true
            } as ImageSettings;
            
        default:
            return {};
    }
}
