<script lang="ts">
    import { lang } from '../i18n';
    export let value: number = 80;
    // Đổi type thành string để có thể chứa trạng thái 'custom' khi user tự kéo lệch mốc
    export let activeTab: string = 'balance'; 

    // Định nghĩa mức % cho từng preset
    const PRESETS = {
        low: 40,
        balance: 80,
        high: 100
    };

    // Khi click vào Button Preset
    function setPreset(tab: 'low' | 'balance' | 'high') {
        activeTab = tab;
        value = PRESETS[tab];
    }

    // Tự động map ngược lại: Cập nhật activeTab khi value thay đổi (từ slider kéo)
    $: {
        if (value === PRESETS.low) activeTab = 'low';
        else if (value === PRESETS.balance) activeTab = 'balance';
        else if (value === PRESETS.high) activeTab = 'high';
        else activeTab = 'custom';
    }
</script>

<div class="control-group">
    <div class="control-label"><span>{$lang.qualityPreset}</span></div>
    <div class="quality-box">
        <div class="preset-tabs">
            <button class="preset-tab {activeTab === 'low' ? 'active' : ''}" on:click={() => setPreset('low')}>{$lang.presetLow}</button>
            <button class="preset-tab {activeTab === 'balance' ? 'active' : ''}" on:click={() => setPreset('balance')}>{$lang.presetBalance}</button>
            <button class="preset-tab {activeTab === 'high' ? 'active' : ''}" on:click={() => setPreset('high')}>{$lang.presetHigh}</button>
        </div>
        <div class="range-inner">
            <input type="range" class="range-slider" min="1" max="100" bind:value={value}>
            <span class="range-val">{value}%</span>
        </div>
    </div>
</div>
