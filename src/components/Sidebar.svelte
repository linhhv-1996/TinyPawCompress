<script lang="ts">
    import { createEventDispatcher, onMount } from "svelte";
    import { lang } from "../i18n";
    import ToggleSwitch from "./ToggleSwitch.svelte";
    import ToggleRow from "./ToggleRow.svelte";
    import PillGroup from "./PillGroup.svelte";
    import QualityPreset from "./QualityPreset.svelte";
    import InputGroup from "./InputGroup.svelte";
    import { downloadDir } from "@tauri-apps/api/path";

    import { open as openDialog } from "@tauri-apps/plugin-dialog";

    export let file: any = null;
    export let filesCount: number = 0; // Đã bỏ applyToAll
    export let isCompressingAll: boolean = false;

    const dispatch = createEventDispatcher();

    $: activePanel = file ? file.file_type : "empty";

    $: if (file && file.file_type === "pdf" && file.settings) {
        if (!file.settings.unlockPdf && file.settings.password !== "") {
            file.settings.password = "";
        }
    }

    // --- LOGIC MỚI CHO NÚT BẤM ---
    let isApplied = false;

    // BIẾN LƯU THƯ MỤC OUTPUT
    let outputDirPath = ""; 
    let outputDirName = "Downloads";

    onMount(async () => {
        // Lấy thư mục Downloads mặc định của máy khi vừa load
        try {
            outputDirPath = await downloadDir();
        } catch (e) {
            console.error(e);
        }
    });

    // Hàm mở cửa sổ chọn thư mục
    async function handleSelectFolder() {
        // Gọi openDialog thay vì open
        const selectedPath = await openDialog({
            directory: true,
            multiple: false,
            defaultPath: outputDirPath,
        });

        if (selectedPath && typeof selectedPath === 'string') {
            outputDirPath = selectedPath;
            // Lấy tên thư mục cuối cùng để hiển thị lên UI cho gọn (vd: "Documents")
            outputDirName = selectedPath.split(/[/\\]/).pop() || selectedPath;
        }
    }


    function handleCompress() {
        dispatch("compress", { outputDir: outputDirPath }); 
    }

    function handleApplyToAll() {
        if (!file || !file.settings) return;

        // Chủ động bắn event copy settings sang các file cùng loại
        dispatch("syncBatch", {
            type: file.file_type,
            settings: file.settings,
        });

        // Hiển thị feedback UI thành công trong 1.5 giây
        isApplied = true;
        setTimeout(() => {
            isApplied = false;
        }, 1500);
    }

    function handleCancel() {
        dispatch("cancel");
    }

</script>

<aside class="sidebar">
    <div class="sidebar-scroll" style="{isCompressingAll ? 'pointer-events: none; opacity: 0.6;' : ''}">
        {#if activePanel === "empty"}
            <div
                class="panel active"
                style="height: 100%; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center;"
            >
                <i
                    class="ph ph-arrow-left"
                    style="font-size: 40px; margin-bottom: 12px;"
                ></i>
                <p
                    style="font-size: 13px; font-weight: 500; color: var(--text-dim); line-height: 1.5; padding: 0 20px;"
                >
                    {$lang.selectFileToView}
                </p>
            </div>
        {/if}

        {#if activePanel === "video" && file.settings}
            {@const hasTargetSize = file.settings.targetSize && parseFloat(file.settings.targetSize) > 0}
            
            <div class="panel active">
                <div class="side-header"><h4>{$lang.videoSettings}</h4></div>

                <!-- <div style="transition: 0.2s; {hasTargetSize ? 'opacity: 0.35; pointer-events: none; filter: grayscale(1);' : ''}">
                    <QualityPreset
                        bind:value={file.settings.qualityValue}
                        bind:activeTab={file.settings.qualityTab}
                    />
                </div> -->
                <div style="transition: 0.2s; {hasTargetSize ? 'opacity: 0.35; pointer-events: none; filter: grayscale(1);' : ''}">
                    <PillGroup
                        label={$lang.qualityProfile}
                        bind:activeId={file.settings.profile}
                        options={[
                            { id: "low", label: "Low" },
                            { id: "balance", label: "Balance" },
                            { id: "high", label: "High" },
                        ]}
                    />
                </div>
        
                <InputGroup
                    label={$lang.targetSize}
                    placeholder={$lang.maxWSizePlaceholder}
                    bind:value={file.settings.targetSize}
                    type="number"
                />

                <!-- <PillGroup
                    label={$lang.resolution}
                    bind:activeId={file.settings.resolution}
                    options={[
                        { id: "original", label: $lang.resOriginal },
                        { id: "1080p", label: $lang.res1080p },
                        { id: "720p", label: $lang.res720p },
                    ]}
                /> -->

                <PillGroup
                    label={$lang.videoCodec}
                    bind:activeId={file.settings.codec}
                    options={[
                        { id: "h264", label: "H.264" },
                        { id: "hevc", label: "HEVC" },
                    ]}
                />

                <ToggleRow
                    iconClass="ph ph-speaker-slash"
                    title={$lang.muteAudio}
                    subtitle={$lang.muteAudioDesc}
                    bind:checked={file.settings.muteAudio}
                />
            </div>
        {/if}

        {#if activePanel === "pdf" && file.settings}
            <div class="panel active">
                <div class="side-header"><h4>{$lang.pdfSettings}</h4></div>

                <PillGroup
                    label={$lang.qualityProfile}
                    bind:activeId={file.settings.profile}
                    options={[
                        { id: "screen", label: $lang.profileScreen },
                        { id: "ebook", label: $lang.profileEbook },
                        { id: "printer", label: $lang.profilePrinter },
                    ]}
                />

                <ToggleRow
                    iconClass="ph ph-palette"
                    title={$lang.grayscale}
                    subtitle={$lang.grayscaleDesc}
                    bind:checked={file.settings.grayscale}
                />
                <ToggleRow
                    iconClass="ph ph-broom"
                    title={$lang.stripMeta}
                    subtitle={$lang.stripMetaDesc}
                    bind:checked={file.settings.stripMeta}
                />

                <div class="control-group" style="margin-top: 12px;">
                    <ToggleRow 
                        iconClass="ph ph-lock-key-open" 
                        title="Unlock PDF" 
                        subtitle="Provide password if PDF is locked" 
                        bind:checked={file.settings.unlockPdf} 
                        hasInput={file.settings.unlockPdf} 
                    />
                    <div class="conditional-wrap {file.settings.unlockPdf ? 'open' : ''}">
                        <div class="conditional-input">
                            <input
                                type="password"
                                class="mac-input"
                                placeholder="Enter password to unlock..."
                                bind:value={file.settings.password}
                            />
                        </div>
                    </div>
                </div>

            </div>
        {/if}

        {#if activePanel === "image" && file.settings}
            <div class="panel active">
                <div class="side-header"><h4>{$lang.imageSettings}</h4></div>

                <QualityPreset
                    bind:value={file.settings.qualityValue}
                    bind:activeTab={file.settings.qualityTab}
                />
                <InputGroup
                    label={$lang.maxWidth}
                    placeholder={$lang.maxWidthPlaceholder}
                    bind:value={file.settings.maxWidth}
                    type="number"
                />

                <PillGroup
                    label={$lang.convertFormat}
                    bind:activeId={file.settings.format}
                    options={[
                        { id: "original", label: $lang.formatOriginal },
                        { id: "jpeg", label: $lang.formatJpeg },
                        { id: "webp", label: $lang.formatWebp },
                    ]}
                />

                <ToggleRow
                    iconClass="ph ph-map-pin-line"
                    title={$lang.stripExif}
                    subtitle={$lang.stripExifDesc}
                    bind:checked={file.settings.stripExif}
                />
            </div>
        {/if}

        {#if activePanel !== "empty" && filesCount > 1}
            <div style="margin-top: 16px; padding-top: 16px; border-top: 1px dashed var(--border);">
                <button
                    class="batch-btn {isApplied ? 'success' : ''}"
                    on:click={handleApplyToAll}
                    style="margin-bottom: 0;"
                >
                    {#if isApplied}
                        <i class="ph ph-check-circle" style="font-size: 14px;"></i>
                        <span>Applied to all {file.file_type.toUpperCase()}s!</span>
                    {:else}
                        <i class="ph ph-copy" style="font-size: 14px;"></i>
                        <span>Apply to all {file.file_type.toUpperCase()}s</span>
                    {/if}
                </button>
            </div>
        {/if}
        
    </div>

    {#if activePanel !== "empty"}
        <div class="sidebar-footer">
            <div class="output-row" style="{isCompressingAll ? 'pointer-events: none; opacity: 0.5;' : ''}">
                <span class="output-label">{$lang.saveTo}</span>
                <button class="output-btn" on:click={handleSelectFolder} disabled={isCompressingAll}>
                    <i class="ph ph-folder-open left-icon"></i>
                    <span class="output-text">{outputDirName}</span>
                    <i class="ph ph-caret-down right-icon"></i>
                </button>
            </div>

            {#if isCompressingAll}
                <button class="btn-compress cancel-btn" on:click={handleCancel}>
                    <i class="ph ph-x-circle" style="font-size: 16px;"></i>
                    Cancel
                </button>
            {:else}
                <button class="btn-compress" on:click={handleCompress}>
                    <i class="ph ph-archive-box" style="font-size: 16px;"></i>
                    Compress {filesCount} Files
                </button>
            {/if}

        </div>
    {/if}
</aside>

<style>
    .btn-compress.cancel-btn {
        background: #EF4444; /* Đỏ Tailwind */
        box-shadow: 0 2px 4px rgba(239, 68, 68, 0.2);
    }
    .btn-compress.cancel-btn:hover {
        background: #DC2626;
        box-shadow: 0 4px 6px rgba(239, 68, 68, 0.25);
    }
</style>
