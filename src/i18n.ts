// src/lib/i18n.ts
import { writable, derived } from 'svelte/store';

// 1. Khai báo các gói ngôn ngữ
const en = {
    appName: "TinyPaw Compressor",
    dragDropText: "Drop files here to compress",
    filesReady: "Files",
    clearAll: "Clear all",
    applyToAll: "Apply to all files",
    currentBatch: "Current: Video (nature_cinematic.mp4)",
    
    // Status
    compressing: "Compressing...",
    ready: "Ready",
    done: "Done",
    
    // Sidebar Empty
    selectFileToView: "Drag & drop files into the main window, or select a file from the list to view its settings.",
    emptyMainHint: "Ready to squish? Drag and drop your videos, PDFs, or images here to get started.",
    
    // Video Panel
    videoSettings: "Video Settings",
    qualityPreset: "Quality preset",
    presetLow: "Low",
    presetBalance: "Balance",
    presetHigh: "High",
    targetSize: "Target Size (MB)",
    resolution: "Resolution",
    resOriginal: "Original",
    res1080p: "1080p",
    res720p: "720p",
    videoCodec: "Video Codec",
    muteAudio: "Mute Audio",
    muteAudioDesc: "Remove sound track",
    maxWSizePlaceholder: "Eg. 50",
    
    // PDF Panel
    pdfSettings: "PDF Settings",
    qualityProfile: "Quality Profile",
    profileScreen: "Screen",
    profileEbook: "Ebook",
    profilePrinter: "Printer",
    grayscale: "Grayscale",
    grayscaleDesc: "Convert to B&W",
    stripMeta: "Strip Metadata",
    stripMetaDesc: "Remove invisible junk",
    passwordProtect: "Password protect",
    passwordProtectDesc: "Encrypt output PDF",
    passwordPlaceholder: "Enter secure password...",
    
    // Image Panel
    imageSettings: "Image Settings",
    maxWidth: "Max Width (px)",
    maxWidthPlaceholder: "Empty for original",
    convertFormat: "Convert Format",
    formatOriginal: "Original",
    formatJpeg: "JPEG",
    formatWebp: "WebP",
    stripExif: "Strip EXIF Data",
    stripExifDesc: "Remove GPS, Camera etc.",
    
    // Footer
    saveTo: "Save to:",
    downloads: "Downloads",
    compressBtn: "Compress 3 Files",

    // Settings Sidebar
    settingsTitle: "Preferences",
    sectionGeneral: "General",
    sectionOutput: "Output",
    
    language: "Language",
    
    licenseKey: "License Key",
    licensePlaceholder: "Enter license key...",
    
    outputDir: "Default Output Folder",
    outDirSource: "Same as original folder",
    outDirDownloads: "Downloads folder",
    outDirCustom: "Choose custom folder...",
    
    fileSuffix: "Filename Suffix",
    suffixPlaceholder: "e.g., _min, _compressed",
    
    saveSettings: "Save",
    cancelBtn: "Cancel",
};

const translations: Record<string, any> = { en };

// 2. State lưu ngôn ngữ hiện tại (mặc định 'en')
export const locale = writable('en');

// 3. Store dẫn xuất: mỗi khi `locale` đổi, `$lang` sẽ tự động cập nhật object tương ứng
export const lang = derived(locale, ($locale) => translations[$locale]);


