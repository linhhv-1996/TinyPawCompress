<script lang="ts">
    import { createEventDispatcher } from 'svelte';
    import { lang } from '../i18n';

    export let type: 'video' | 'pdf' | 'image';
    export let isActive: boolean = false;
    export let isProcessing: boolean = false;
    export let isGlobalProcessing: boolean = false;
    export let isDone: boolean = false;

    export let thumbUrl: string = '';
    export let thumbIconClass: string = '';
    export let thumbIconColor: string = '';
    export let thumbBg: string = '#F9FAFB';

    export let badgeBg: string;
    export let badgeColor: string;
    export let badgeText: string;

    export let filename: string;
    export let beforeStat: string;
    export let afterStat: string = '';
    export let savedPercent: string = '';
    export let infoIcon: string;
    export let infoText: string;

    const dispatch = createEventDispatcher();
    
</script>

<div class="file-card {isActive ? 'active' : ''} {isProcessing ? 'processing' : ''} {isDone ? 'done' : ''}">
    <!-- svelte-ignore a11y_consider_explicit_label -->
    {#if !isGlobalProcessing}
        <button class="btn-delete" on:click|stopPropagation={() => dispatch('delete')}>
            <i class="ph-fill ph-x-circle"></i>
        </button>
    {/if}
    
    <div class="thumb" style="background: {thumbBg};">
        {#if thumbUrl}
            <img src={thumbUrl} alt="thumb">
        {:else}
            <i class="{thumbIconClass}" style="font-size: 32px; color: {thumbIconColor};"></i>
        {/if}
        
        {#if isProcessing}
            <div class="overlay" style="display: flex; align-items: center; justify-content: center; gap: 6px; opacity: 1; background: rgba(255,255,255,0.7); height: 100%; width: 100%;">
                <i class="ph ph-spinner-gap spinner" style="font-size: 32px;font-weight:bold;"></i>
                <span style="font-size: 14px; font-weight: 700; line-height: 1;">{$lang.compressing}</span>
            </div>
        {/if}
    </div>

    <div class="card-info">
        <div class="badge-wrap">
            <span class="type-badge" style="background: {badgeBg}; color: {badgeColor};">
                {badgeText}
            </span>
        </div>

        <div class="fname">{filename}</div>
        <div class="fmeta">
            <div>
                <span class="before-stat">{beforeStat}</span>
                {#if isDone && afterStat}
                    <span class="after-stat" style="color: #059669; font-weight: 600;">
                        ➔ {afterStat}
                        {#if savedPercent}
                            <span class="badge-saved" style="margin-left: 4px; font-size: 11px; background: #D1FAE5; padding: 2px 4px; border-radius: 4px;">
                                {savedPercent}
                            </span>
                        {/if}
                    </span>
                {/if}
            </div>
            <div class="res-info">
                <i class="{infoIcon}"></i> {infoText}
            </div>
        </div>
    </div>
</div>

<style>
    .spinner {
        animation: spin 1s linear infinite;
    }
    @keyframes spin {
        from { transform: rotate(0deg); }
        to { transform: rotate(360deg); }
    }
</style>
