<script lang="ts">
    export let label: string;
    export let placeholder: string = '';
    export let value: string | number = '';
    export let type: string = 'text';
</script>

<div class="control-group">
    <div class="control-label"><span>{label}</span></div>
    <input {type} class="mac-input" {placeholder} bind:value={value}>
</div>
