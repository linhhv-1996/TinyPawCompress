<script lang="ts">
    export let checked: boolean = false;
</script>

<label class="switch">
    <input type="checkbox" bind:checked={checked} />
    <span class="slider"></span>
</label>
