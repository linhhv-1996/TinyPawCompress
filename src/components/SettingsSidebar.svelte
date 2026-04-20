<script lang="ts">
    import { createEventDispatcher } from 'svelte';
    import { lang, locale } from '../i18n';

    const dispatch = createEventDispatcher();
    
    // State dữ liệu setting
    let licenseKey = '';
    let outputDir = 'source';
    let suffix = '_min';

    function changeLanguage(event: Event) {
        const target = event.target as HTMLSelectElement;
        locale.set(target.value);
    }

    function handleSave() {
        // Xử lý lưu state vào LocalStorage hoặc Rust ở đây
        dispatch('close');
    }

    function handleCancel() {
        dispatch('close');
    }
</script>

<aside class="sidebar settings-sidebar">
    <div class="sidebar-scroll">
        <div class="side-header">
            <h4>{$lang.settingsTitle}</h4>
        </div>

        <div class="setting-section">
            
            <div class="control-group">
                <div class="control-label">{$lang.language}</div>
                <select class="mac-input" on:change={changeLanguage} value={$locale}>
                    <option value="en">English</option>
                    <option value="vi">Tiếng Việt</option>
                </select>
            </div>

            <div class="control-group">
                <div class="control-label">{$lang.licenseKey}</div>
                <input type="text" class="mac-input" placeholder={$lang.licensePlaceholder} bind:value={licenseKey} />
            </div>
        </div>

        <div class="setting-section">
            
            <div class="control-group">
                <div class="control-label">{$lang.outputDir}</div>
                <select class="mac-input" bind:value={outputDir}>
                    <option value="source">{$lang.outDirSource}</option>
                    <option value="downloads">{$lang.outDirDownloads}</option>
                    <option value="custom">{$lang.outDirCustom}</option>
                </select>
            </div>

            <div class="control-group">
                <div class="control-label">{$lang.fileSuffix}</div>
                <input type="text" class="mac-input" placeholder={$lang.suffixPlaceholder} bind:value={suffix} />
            </div>
        </div>
    </div>
    
    <div class="sidebar-footer" style="display: flex; gap: 8px;">
        <button class="btn-cancel" on:click={handleCancel}>
            {$lang.cancelBtn}
        </button>
        <button class="btn-compress" style="flex: 1;" on:click={handleSave}>
            <i class="ph ph-check"></i> {$lang.saveSettings}
        </button>
    </div>
</aside>

<style>
    .settings-sidebar { 
        background: #FAFAFA; 
    }

    select.mac-input { 
        appearance: none; 
        cursor: pointer; 
    }
    
    .setting-section {
        margin-bottom: 12px;
        padding-bottom: 12px;
        border-bottom: 1px dashed var(--border);
    }
    .setting-section:last-child {
        border-bottom: none;
        margin-bottom: 0;
    }

    .btn-cancel {
        background: #FFFFFF; 
        border: 1px solid var(--border); 
        padding: 10px 16px;
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
        border-color: #9CA3AF; 
    }
</style>
