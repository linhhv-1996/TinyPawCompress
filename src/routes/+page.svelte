<script lang="ts">
    import { onMount } from "svelte";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import { invoke, convertFileSrc } from "@tauri-apps/api/core"; // convertFileSrc để hiện ảnh từ ổ cứng
    import { lang } from "../i18n";
    import FileCard from "../components/FileCard.svelte";
    import Sidebar from "../components/Sidebar.svelte";
    import SettingsSidebar from "../components/SettingsSidebar.svelte";
    import { getDefaultSettings } from "../config/settings";
    import { join } from "@tauri-apps/api/path";
    import pLimit from 'p-limit';

    // State quản lý danh sách file và UI
    let files: any[] = [];
    let selectedId: string | null = null;
    let isDragging = false;
    let showSettings = false;
    let isCompressingAll = false;

    const limit = pLimit(1);
    let pendingThumbs = new Map<string, string>();

    // Cấu hình màu sắc/icon theo loại file
    const typeConfig: any = {
        video: { bg: "#E1EFFE", color: "#1E40AF", icon: "ph ph-video-camera" },
        pdf: { bg: "#FEE2E2", color: "#991B1B", icon: "ph ph-file-pdf" },
        image: { bg: "#FEF3C7", color: "#92400E", icon: "ph ph-image" },
        audio: { bg: "#D1FAE5", color: "#065F46", icon: "ph ph-speaker-high" },
    };

    // Phản ứng: Tự động tìm file đang được chọn
    $: selectedIndex = files.findIndex((f) => f.id === selectedId);

    onMount(() => {
        let unlistenFunctions: UnlistenFn[] = [];

        async function setupListeners() {
            // 1. Lắng nghe file được thả vào
            const drop = await listen(
                "tauri://drag-drop",
                async (event: any) => {
                    isDragging = false;
                    if (isCompressingAll) return;
                    const payload = event.payload as { paths: string[] };
                    if (payload.paths.length > 0) {
                        await handleNewFiles(payload.paths);
                    }
                },
            );
            unlistenFunctions.push(drop);

            // 2. Lắng nghe thumbnail gửi từ Rust (cho Image/PDF)
            const thumbListener = await listen(
                "thumbnail-ready",
                (event: any) => {
                    const { id, data } = event.payload as { id: string; data: string; };
                    
                    // Check xem file đã kịp xuất hiện trong danh sách chưa
                    const isExist = files.some((f) => f.id === id);
                    if (isExist) {
                        updateFileThumbnail(id, data); // Đã có file -> Cập nhật luôn
                    } else {
                        pendingThumbs.set(id, data);   // Chưa có file -> Nhét vào phòng chờ
                    }
                },
            );
            
            unlistenFunctions.push(thumbListener);

            // 3. Hiệu ứng kéo thả UI
            const enter = await listen(
                "tauri://drag-enter",
                () => (isDragging = true),
            );
            const leave = await listen(
                "tauri://drag-leave",
                () => (isDragging = false),
            );
            unlistenFunctions.push(enter, leave);
        }

        setupListeners();
        return () => unlistenFunctions.forEach((fn) => fn());
    });

    // Hàm xử lý chính khi có file mới
    async function handleNewFiles(paths: string[]) {
        const existingPaths = new Set(files.map(f => f.path));
        const uniquePaths = paths.filter(p => !existingPaths.has(p));

        if (uniquePaths.length === 0) return;

        // Gọi Rust lấy thông tin cơ bản ngay lập tức (không đợi thumbnail)
        const newFiles: any[] = await invoke("handle_dropped_files", { paths: uniquePaths });

        const filesWithSettings = newFiles.map((f) => ({
            ...f,
            settings: getDefaultSettings(f.file_type),
        }));

        // Cập nhật danh sách hiển thị ngay để user không phải chờ
        files = [...files, ...filesWithSettings];

        files = files.map((f) => {
            if (pendingThumbs.has(f.id)) {
                const thumbData = pendingThumbs.get(f.id);
                pendingThumbs.delete(f.id); // Lấy xong thì xóa khỏi phòng chờ
                return { ...f, thumbnail: thumbData };
            }
            return f;
        });

        if (!selectedId && files.length > 0) selectedId = files[0].id;

        // Với Video: Tự xử lý thumbnail ở FE để giảm tải cho Rust
        newFiles.forEach(async (file) => {
            if (file.file_type === "video") {
                const thumb = await createVideoThumbnail(file.path);
                if (thumb) updateFileThumbnail(file.id, thumb);
            }
        });
    }

    // Hàm tạo thumbnail video bằng Canvas (Chạy ngầm)
    async function createVideoThumbnail(path: string): Promise<string> {
        return new Promise((resolve) => {
            const video = document.createElement("video");
            video.src = convertFileSrc(path);
            video.currentTime = 1; // Lấy khung hình ở giây thứ 1
            video.muted = true;

            video.onloadeddata = () => {
                const canvas = document.createElement("canvas");
                // Scale nhỏ lại cho nhẹ RAM
                canvas.width = 320;
                canvas.height = (video.videoHeight / video.videoWidth) * 320;

                const ctx = canvas.getContext("2d");
                ctx?.drawImage(video, 0, 0, canvas.width, canvas.height);
                resolve(canvas.toDataURL("image/jpeg", 0.7)); // Nén 70%
            };
            video.onerror = () => resolve("");
        });
    }

    // Cập nhật thumbnail vào mảng dữ liệu
    const updateFileThumbnail = (id: string, data: string) => {
        files = files.map((f) => (f.id === id ? { ...f, thumbnail: data } : f));
    };

    const clearAll = () => {
        if (isCompressingAll) return;
        files = [];
        selectedId = null;
    };

    function handleSyncBatch(event: CustomEvent) {
        const { type, settings } = event.detail;
        files = files.map((f) => {
            if (f.file_type === type && f.id !== selectedId) {
                // Clone object ra để tránh bị tham chiếu vùng nhớ
                return { ...f, settings: JSON.parse(JSON.stringify(settings)) };
            }
            return f;
        });
    }

    // Thêm thư viện xử lý đường dẫn nếu cần, hoặc dùng regex cơ bản
    async function startCompression(event: CustomEvent<{ outputDir: string }>) {
        console.log("🚀 Bắt đầu nén...");
        isCompressingAll = true; 
        const outputDir = event.detail.outputDir;
        console.log("🚀 Thư mục đích:", outputDir);

        // 1. Đổi UI sang trạng thái "compressing" cho file PDF và Video
        files = files.map((f) => {
            if (f.file_type === "pdf" || f.file_type === "video" || f.file_type === "image") {
                return { ...f, status: "compressing", afterStat: '', savedPercent: '' };
            }
            return f;
        });

        // 2. Chạy hàng đợi p-limit
        const compressTasks = files.map((file) => {
            if (file.file_type !== "pdf" && file.file_type !== "video" && file.file_type !== "image") return Promise.resolve();

            return limit(async () => {
                if (!isCompressingAll) {
                    updateFileUI(file.id, "error", "Cancelled");
                    return;
                }

                const oldNameStr = file.name.replace(/\.[^/.]+$/, "");
                let ext = file.name.split('.').pop();

                if (file.file_type === "image") {
                    if (file.settings.format === "jpeg") ext = "jpg";
                    else if (file.settings.format === "webp") ext = "webp";
                }

                const newFileName = `${oldNameStr}_compressed.${ext}`;

                try {
                    const outPath = await join(outputDir, newFileName);
                    let res: any;

                    // PHÂN NHÁNH GỌI RUST THEO LOẠI FILE
                    if (file.file_type === "pdf") {
                        res = await invoke("compress_pdf_command", {
                            id: file.id,
                            inputPath: file.path,
                            outputPath: outPath,
                            profile: file.settings.profile || "ebook",
                            grayscale: file.settings.grayscale || false,
                            stripMeta: file.settings.stripMeta || false,
                        });
                    } else if (file.file_type === "video") {
                        res = await invoke("compress_video_command", {
                            id: file.id,
                            inputPath: file.path,
                            outputPath: outPath,
                            profile: file.settings.profile || "balance",
                            targetSize: parseFloat(file.settings.targetSize) || 0.0,
                            codec: file.settings.codec || "h264",
                            muteAudio: file.settings.muteAudio || false,
                        });
                    } else if (file.file_type === "image") {
                        // GỌI XUỐNG RUST COMMAND NÉN ẢNH
                        res = await invoke("compress_image_command", {
                            id: file.id,
                            inputPath: file.path,
                            outputPath: outPath,
                            qualityValue: file.settings.qualityValue || 80,
                            maxWidth: file.settings.maxWidth.toString(),
                            format: file.settings.format || "original",
                            stripExif: file.settings.stripExif !== undefined ? file.settings.stripExif : true,
                        });
                    }

                    // Cập nhật kết quả lên UI
                    if (res && res.success) {
                        updateFileUI(file.id, "done", res.new_size_text, res.new_size_bytes);
                    } else if (res && !res.success) {
                        updateFileUI(file.id, "error", res.error_msg);
                    }
                } catch (err) {
                    updateFileUI(file.id, "error", String(err));
                }
            });
        });

        await Promise.all(compressTasks);
        isCompressingAll = false;
        console.log("✅ Hoàn tất toàn bộ danh sách!");
    }


    async function cancelCompression() {
        isCompressingAll = false; // Tắt cờ, các task đang chờ trong p-limit sẽ tự hủy
        
        // Gửi lệnh Cancel tới Rust cho những file ĐANG CHẠY
        for (const file of files) {
            if (file.status === "compressing") {
                // Truyền thêm fileType xuống Rust
                await invoke("cancel_compression_command", { 
                    id: file.id, 
                    fileType: file.file_type 
                });
            }
        }
    }

    // Hàm helper để update mảng dữ liệu svelte trigger UI update
    function updateFileUI(id: any, status: any, extraData: any, extraBytes?: number) {
        files = files.map((f) => {
            if (f.id === id) {
                if (status === "done") {
                    let savedPercent = "";
                    if (extraBytes && f.size_bytes > 0) {
                        const pct = Math.round(((f.size_bytes - extraBytes) / f.size_bytes) * 100);
                        if (pct > 0) savedPercent = `-${pct}%`;
                    }
                    
                    return {
                        ...f,
                        status: "done",
                        afterStat: extraData,      // MB sau nén
                        savedPercent: savedPercent, // % tiết kiệm
                        // dynamicBadgeText: "DONE",
                        // dynamicBadgeBg: "#D1FAE5", // Xanh lá
                        // dynamicBadgeColor: "#065F46",
                    };
                } else if (status === "error") {
                    if (extraData === "Cancelled") {
                        return {
                            ...f,
                            status: "ready",
                        };
                    }

                    return {
                        ...f,
                        status: "error",
                        metadata: `Lỗi: ${extraData}`, // Ghi đè dòng metadata bằng lỗi
                        // dynamicBadgeText: "ERROR",
                        // dynamicBadgeBg: "#FEE2E2", // Đỏ
                        // dynamicBadgeColor: "#991B1B",
                    };
                }
            }
            return f;
        });
    }

    function removeFile(id: string) {
        files = files.filter(f => f.id !== id);
        
        // Nếu file đang xoá là file đang được chọn, hãy chọn file đầu tiên hoặc null
        if (selectedId === id) {
            selectedId = files.length > 0 ? files[0].id : null;
        }
    }

</script>

<div class="window">
    <div class="drag-overlay {isDragging ? 'active' : ''}">
        <div class="drag-content">
            <i class="ph ph-files"></i>
            <p>{$lang.dragDropText}</p>
        </div>
    </div>

    <main class="main-content">
        <div class="title-bar" data-tauri-drag-region>
            {$lang.appName}
            <!-- svelte-ignore a11y_consider_explicit_label -->
            <button
                class="title-btn-settings {showSettings ? 'active' : ''}"
                on:click={() => { if (!isCompressingAll) showSettings = !showSettings }}
                style="{isCompressingAll ? 'opacity: 0.5; cursor: not-allowed;' : ''}"
            >
                <i class="ph ph-gear"></i>
            </button>
        </div>

        {#if files.length === 0}
            <div class="empty-hero">
                <div class="hero-box">
                    <div class="hero-icon">
                        <i class="ph ph-hand-grabbing"></i>
                    </div>
                    <h2>{$lang.emptyMainHint}</h2>
                    <p>Supports MP4, PDF, JPG, PNG and more</p>
                </div>
            </div>
        {:else}
            <div class="grid-header">
                <span>{files.length} {$lang.filesReady}</span>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div class="clear-all"
                    on:click={clearAll}
                    style="{isCompressingAll ? 'opacity: 0.5; cursor: not-allowed;' : ''}"
                >
                    {$lang.clearAll}
                </div>
            </div>

            <div class="grid">
                {#each files as file (file.id)}
                    {@const cfg =
                        typeConfig[file.file_type] || typeConfig.video}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <div
                        on:click={() => (selectedId = file.id)}
                        role="button"
                        tabindex="0"
                        style="opacity: {file.status === 'compressing'
                            ? 0.6
                            : 1}; transition: 0.3s;"
                    >
                        <FileCard
                            type={file.file_type}
                            isActive={selectedId === file.id}
                            isDone={file.status === 'done'}
                            isProcessing={file.status === 'compressing'}
                            isGlobalProcessing={isCompressingAll}
                            filename={file.name}
                            beforeStat={file.size_text}
                            afterStat={file.afterStat}
                            savedPercent={file.savedPercent}
                            badgeBg={file.dynamicBadgeBg || cfg.bg}
                            badgeColor={file.dynamicBadgeColor || cfg.color}
                            badgeText={file.file_type.toUpperCase()}
                            infoIcon={cfg.icon}
                            infoText={file.metadata}
                            thumbIconClass={cfg.icon}
                            thumbIconColor={cfg.color}
                            thumbBg={cfg.bg}
                            thumbUrl={file.thumbnail}
                            on:delete={() => removeFile(file.id)}
                        />
                    </div>
                {/each}
            </div>
        {/if}
    </main>

    {#if showSettings}
        <SettingsSidebar on:close={() => (showSettings = false)} />
    {:else if selectedIndex !== -1}
        <Sidebar
            bind:file={files[selectedIndex]}
            filesCount={files.length}
            isCompressingAll={isCompressingAll} 
            on:syncBatch={handleSyncBatch}
            on:compress={startCompression}
            on:cancel={cancelCompression}
        />
    {:else}
        <Sidebar />
    {/if}
</div>

<style>
    /* CSS cho vùng trung tâm mới */
    .empty-hero {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 40px;
    }
    .hero-box {
        text-align: center;
        max-width: 420px;
    }
    .hero-icon {
        font-size: 48px;
        color: var(--accent);
        background: #eef2ff;
        width: 80px;
        height: 80px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 20px;
        margin: 0 auto 24px auto;
        box-shadow: 0 4px 12px rgba(94, 92, 230, 0.1);
    }
    .hero-box h2 {
        font-size: 17px;
        font-weight: 600;
        line-height: 1.6;
        color: var(--text-main);
    }
    .hero-box p {
        font-size: 13px;
        color: var(--text-dim);
        margin-top: 10px;
    }
    .clear-all {
        cursor: pointer;
    }
</style>
