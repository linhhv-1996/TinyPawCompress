<script lang="ts">
    import { onMount } from "svelte";
    import { fly } from "svelte/transition";

    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import { invoke, convertFileSrc } from "@tauri-apps/api/core"; // convertFileSrc để hiện ảnh từ ổ cứng
    import { lang } from "../i18n";
    import FileCard from "../components/FileCard.svelte";
    import Sidebar from "../components/Sidebar.svelte";
    import { getDefaultSettings } from "../config/settings";
    import { join } from "@tauri-apps/api/path";
    import pLimit from 'p-limit';
   import { openUrl } from "@tauri-apps/plugin-opener";

    // State quản lý danh sách file và UI
    let files: any[] = [];
    let selectedId: string | null = null;
    let isDragging = false;
    let isCompressingAll = false;

    let isPro = false;
    let showProModal = false;
    let licenseInput = "";
    let licenseError = "";
    let isVerifying = false;

    let showSettingsMenu = false;

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
        invoke("get_pro_status").then((config: any) => {
            isPro = config.is_pro;
        }).catch(console.error);

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

    let toast = { show: false, message: "", type: "success" };
    let toastTimeout: any;

    function showToast(message: string, type: "success" | "info" | "error" = "success") {
        if (toastTimeout) clearTimeout(toastTimeout); // Reset thời gian nếu bấm liên tục
        toast = { show: true, message, type };
        toastTimeout = setTimeout(() => {
            toast.show = false;
        }, 3000); // Tự động ẩn sau 3 giây
    }

    async function handleOpenExternalLink(url: string) {
        try {
            await openUrl(url);
        } catch (error) {
            console.error("Không thể mở link:", error);
        }
    }

    // Hàm xử lý chính khi có file mới
    async function handleNewFiles(paths: string[]) {
        const existingPaths = new Set(files.map(f => f.path));
        const uniquePaths = paths.filter(p => !existingPaths.has(p));

        if (uniquePaths.length === 0) return;

        if (!isPro && (files.length + uniquePaths.length > 5)) {
            showProModal = true;
            return;
        }

        try {
            const newFiles: any[] = await invoke("handle_dropped_files", { paths: uniquePaths });
            
            const filesWithSettings = newFiles.map((f) => ({
                ...f,
                settings: getDefaultSettings(f.file_type),
            }));

            files = [...files, ...filesWithSettings];

            files = files.map((f) => {
                if (pendingThumbs.has(f.id)) {
                    const thumbData = pendingThumbs.get(f.id);
                    pendingThumbs.delete(f.id);
                    return { ...f, thumbnail: thumbData };
                }
                return f;
            });

            if (!selectedId && files.length > 0) selectedId = files[0].id;

            newFiles.forEach(async (file) => {
                if (file.file_type === "video") {
                    const thumb = await createVideoThumbnail(file.path);
                    if (thumb) updateFileThumbnail(file.id, thumb);
                }
            });

        } catch (err) {
            // Bắt lỗi từ Rust trả về
            if (err === "LIMIT_REACHED") {
                showProModal = true; // Bật Popup Paywall
            } else {
                console.error("Lỗi thêm file:", err);
            }
        }
    }

    // Hàm gọi xuống Rust để kiểm tra Key
    async function submitLicense() {
        if (!licenseInput) return;
        isVerifying = true;
        licenseError = "";
        try {
            const success = await invoke("verify_license", { key: licenseInput });
            if (success) {
                isPro = true;
                showProModal = false;
                showToast("Upgraded to Pro successfully!", "success");
            }
        } catch (err) {
            licenseError = String(err);
        } finally {
            isVerifying = false;
        }
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

        try {
            await invoke("check_compression_limit", { count: files.length });
        } catch (err) {
            if (err === "LIMIT_REACHED" || String(err).includes("LIMIT_REACHED")) {
                showProModal = true;
                return; 
            }
        }


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
                            unlockPdf: file.settings.unlockPdf || false,
                            password: file.settings.password || "",
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
                            maxWidth: file.settings.maxWidth ? file.settings.maxWidth.toString() : "",
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
                        errorMsg: extraData,
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
            <div class="title-actions">
            <button
                class="title-btn {showSettingsMenu ? 'active' : ''}"
                on:click={() => showSettingsMenu = !showSettingsMenu}
                style="{isCompressingAll ? 'opacity: 0.5; pointer-events: none;' : ''}"
                title="Settings & Info"
            >
                <i class="ph ph-gear" style="font-size: 16px;"></i>
            </button>

            {#if showSettingsMenu}
                <div class="menu-overlay" on:click={() => showSettingsMenu = false}></div>
                
                <div class="settings-dropdown">
                    <div class="dropdown-item {isPro ? 'pro-active' : 'pro-upgrade'}"
                        on:click={() => { 
                            showSettingsMenu = false; 
                            if (!isPro) showProModal = true; 
                        }}
                    >
                        {#if isPro}
                            <i class="ph-fill ph-check-circle"></i> <span>Pro Active</span>
                        {:else}
                            <i class="ph-fill ph-crown"></i> <span>Upgrade to Pro</span>
                        {/if}
                    </div>

                    <div class="dropdown-divider"></div>

                    <div class="dropdown-item" on:click={() => { showSettingsMenu = false; showToast('Checking for updates...', 'info'); }}>
                        <i class="ph ph-arrows-clockwise"></i> <span>Check for Updates</span>
                    </div>

                    <div class="dropdown-item" on:click={() => { showSettingsMenu = false; handleOpenExternalLink("https://your-website.com"); }}>
                        <i class="ph ph-info"></i> <span>About TinyPaw</span>
                    </div>
                </div>
            {/if}
        </div>
        </div>

        {#if files.length === 0}
            <div class="empty-hero">
                <div class="hero-box">
                    <div class="hero-icon">
                        <i class="ph ph-hand-grabbing"></i>
                    </div>
                    <h2>{$lang.emptyMainHint}</h2>
                    <p>Supports MP4, MOV, PDF, JPG, PNG, WEBP</p>
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
                            isError={file.status === 'error'} 
                            errorMessage={file.errorMsg}
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

    {#if selectedIndex !== -1}
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


{#if showProModal}
    <div class="modal-overlay">
        <div class="modal-content">
            <div class="modal-header">
                <h2>Upgrade to TinyPaw Pro 🐾</h2>
                <!-- svelte-ignore a11y_consider_explicit_label -->
                <button class="close-btn" on:click={() => showProModal = false} disabled={isVerifying}>
                    <i class="ph ph-x"></i>
                </button>
            </div>
            
            <p class="modal-desc">
                The free version allows processing up to 5 files.<br/>
                You've reached the limit! Upgrade now for unlimited usage.
            </p>
            
            <div class="control-group">
                <div class="control-label" style="display: flex; justify-content: space-between; align-items: center;">
                    <span>Enter License Key:</span>
                    <!-- svelte-ignore a11y_invalid_attribute -->
                    <a 
                        href="#" 
                        class="buy-link" 
                        tabindex="-1"
                        on:click|preventDefault={() => handleOpenExternalLink("https://your-website.com/buy")}
                    >
                        Get a key <i class="ph ph-arrow-up-right"></i>
                    </a>
                </div>
                
                <input 
                    type="text" 
                    class="mac-input" 
                    placeholder="e.g., TINYPAW-PRO" 
                    bind:value={licenseInput}
                    disabled={isVerifying}
                />
                {#if licenseError}
                    <p class="error-text">
                        <i class="ph-fill ph-warning-circle"></i> {licenseError}
                    </p>
                {/if}
            </div>
            
            <div class="modal-actions">
                <button class="btn-cancel" on:click={() => showProModal = false} disabled={isVerifying}>
                    Maybe Later
                </button>
                <button class="btn-compress" on:click={submitLicense} disabled={isVerifying}>
                    {#if isVerifying}
                        <i class="ph ph-spinner-gap spinner"></i> Verifying...
                    {:else}
                        Activate Now
                    {/if}
                </button>
            </div>
        </div>
    </div>
{/if}


{#if toast.show}
    <div class="toast-wrapper" transition:fly={{ y: -20, duration: 300 }}>
        <div class="toast-item {toast.type}">
            {#if toast.type === 'success'}
                <i class="ph-fill ph-check-circle"></i>
            {:else if toast.type === 'info'}
                <i class="ph-fill ph-info"></i>
            {:else}
                <i class="ph-fill ph-warning-circle"></i>
            {/if}
            <span>{toast.message}</span>
        </div>
    </div>
{/if}

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
        background: #F9FAFB; /* Đổi nền xanh #eef2ff sang xám sáng */
        width: 80px;
        height: 80px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 20px;
        margin: 0 auto 24px auto;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05); /* Đổi đổ bóng xanh tím sang đen mờ */
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

    /* CSS CHO MODAL PRO GATE */
    .modal-overlay {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.4);
        z-index: 1000;
        display: flex;
        align-items: center;
        justify-content: center;
        backdrop-filter: blur(4px);
    }
    
    .modal-content {
        background: #FFFFFF;
        padding: 24px;
        border-radius: 12px;
        width: 400px;
        box-shadow: 0 10px 25px rgba(0, 0, 0, 0.1);
        animation: modalFadeIn 0.2s ease-out;
    }
    
    @keyframes modalFadeIn {
        from { opacity: 0; transform: translateY(10px); }
        to { opacity: 1; transform: translateY(0); }
    }

    .modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 12px;
    }
    
    .modal-header h2 {
        font-size: 18px;
        font-weight: 700;
        color: var(--text-main);
    }
    
    .close-btn {
        background: transparent;
        border: none;
        font-size: 20px;
        cursor: pointer;
        color: var(--text-dim);
        transition: color 0.2s;
    }
    
    .close-btn:hover {
        color: var(--text-main);
    }
    
    .modal-desc {
        font-size: 13px;
        color: var(--text-dim);
        margin-bottom: 20px;
        line-height: 1.5;
    }
    
    .error-text {
        color: #DC2626;
        font-size: 12px;
        margin-top: 8px;
        display: flex;
        align-items: center;
        gap: 4px;
    }

    .modal-actions {
        display: flex;
        gap: 10px;
        margin-top: 24px;
    }
    
    .modal-actions button {
        flex: 1;
        padding: 10px 0;
        justify-content: center;
    }
    
    .btn-cancel {
        background: #FFFFFF; 
        border: 1px solid var(--border); 
        border-radius: 6px; 
        font-size: 13px; 
        font-weight: 500; 
        color: var(--text-main);
        cursor: pointer; 
        transition: all 0.2s ease; 
        box-shadow: 0 1px 2px rgba(0,0,0,0.02);
    }
    
    .btn-cancel:hover { 
        background: #F3F4F6; 
    }

    .buy-link {
        font-size: 11px;
        color: var(--text-dim);
        text-decoration: none;
        font-weight: 600;
        display: flex;
        align-items: center;
        gap: 2px;
        transition: color 0.2s ease;
    }
    
    .buy-link:hover {
        color: var(--accent);
        text-decoration: underline;
    }

    /* Container chứa các nút góc phải */
    .title-actions {
        position: absolute;
        right: 12px;
        top: 50%;
        transform: translateY(-50%);
        display: flex;
        gap: 6px; /* Giảm gap một chút cho gọn */
        z-index: 10;
    }

    /* Nút cơ bản (Info/Update) */
    .title-btn {
        background: #FFFFFF;
        border: 1px solid var(--border);
        color: var(--text-dim);
        font-size: 11px;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.2s ease;
        height: 24px;
        padding: 0 8px;
        border-radius: 4px;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 4px;
        box-shadow: 0 1px 2px rgba(0,0,0,0.02);
    }

    .title-btn:hover {
        background: #F3F4F6;
        color: var(--text-main);
    }

    .title-btn i {
        font-size: 14px;
    }

    /* Nếu muốn hiện cả chữ trên các nút thì giữ đoạn này */
    .title-btn .btn-text {
        display: inline; 
    }

    /* Style đặc biệt cho nút PRO */
    .title-btn.pro-btn {
        border-color: #FCD34D; /* Viền vàng nhạt */
        color: #B45309;       /* Chữ cam đậm */
        background: #FFFBEB;  /* Nền vàng siêu nhạt */
    }

    .title-btn.pro-btn:hover {
        background: #FEF3C7;
        color: #92400E;
        border-color: #FBBF24;
    }

    .title-btn.pro-active {
        border-color: #A7F3D0;
        color: #059669;
        background: #ECFDF5;
        pointer-events: none;
    }

    /* TOAST NOTIFICATION */
    .toast-wrapper {
        position: fixed;
        top: 24px;
        left: 50%;
        transform: translateX(-50%);
        z-index: 9999;
        pointer-events: none; /* Tránh block click chuột của user bên dưới */
    }
    
    .toast-item {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 10px 18px;
        border-radius: 30px;
        font-size: 13px;
        font-weight: 600;
        box-shadow: 0 4px 15px rgba(0, 0, 0, 0.1);
        backdrop-filter: blur(10px);
    }
    
    .toast-item i {
        font-size: 16px;
    }
    
    .toast-item.success {
        background: rgba(236, 253, 245, 0.95);
        color: #059669;
        border: 1px solid #A7F3D0;
    }
    
    .toast-item.info {
        background: rgba(243, 244, 246, 0.95);
        color: #374151;
        border: 1px solid #D1D5DB;
    }
    
    .toast-item.error {
        background: rgba(254, 242, 242, 0.95);
        color: #DC2626;
        border: 1px solid #FECACA;
    }

    /* --- CSS CHO DROPDOWN MENU SETTINGS --- */
    .menu-overlay {
        position: fixed;
        top: 0;
        left: 0;
        width: 100vw;
        height: 100vh;
        z-index: 90;
        cursor: default;
    }

    .settings-dropdown {
        position: absolute;
        top: 32px; /* Cách nút bánh răng một chút */
        right: 0;
        background: #FFFFFF;
        border: 1px solid var(--border);
        border-radius: 8px;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.1);
        width: 180px;
        z-index: 100;
        display: flex;
        flex-direction: column;
        padding: 4px;
        animation: menuFadeIn 0.15s ease-out;
    }

    @keyframes menuFadeIn {
        from { opacity: 0; transform: translateY(-5px) scale(0.95); }
        to { opacity: 1; transform: translateY(0) scale(1); }
    }

    .dropdown-item {
        padding: 8px 10px;
        font-size: 13px;
        font-weight: 500;
        color: var(--text-main);
        border-radius: 4px;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 8px;
        transition: background 0.15s ease;
    }

    .dropdown-item:hover {
        background: #F3F4F6;
    }

    .dropdown-item i {
        font-size: 16px;
        color: var(--text-dim);
    }

    .dropdown-divider {
        height: 1px;
        background: var(--border);
        margin: 4px 0;
    }

    /* Làm nổi bật phần PRO trong Menu */
    .dropdown-item.pro-upgrade {
        color: #B45309;
    }
    .dropdown-item.pro-upgrade i {
        color: #D97706;
    }
    .dropdown-item.pro-upgrade:hover {
        background: #FEF3C7;
    }

    .dropdown-item.pro-active {
        color: #059669;
        pointer-events: none; /* Tắt click nếu đã kích hoạt Pro */
    }
    .dropdown-item.pro-active i {
        color: #059669;
    }
</style>
