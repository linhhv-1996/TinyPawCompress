<script lang="ts">
    import ToggleSwitch from './ToggleSwitch.svelte';
    
    export let iconClass: string;
    export let title: string;
    export let subtitle: string;
    export let checked: boolean = false;
    export let hasInput: boolean = false;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div 
    class="toggle-row {hasInput ? 'has-input' : ''}" 
    on:click|preventDefault={() => checked = !checked}
    style="cursor: pointer;"
>
    <div class="toggle-info">
        <i class="{iconClass}"></i>
        <div class="toggle-text">
            <span>{title}</span>
            <small>{subtitle}</small>
        </div>
    </div>
    <ToggleSwitch bind:checked={checked} />
</div>
