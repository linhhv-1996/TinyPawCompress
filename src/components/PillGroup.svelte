<script lang="ts">
    export let label: string;
    export let options: { id: string, label: string }[] = [];
    export let activeId: string;
</script>

<div class="pill-box">
    <span class="pill-label">{label}</span>
    <div class="pill-group">
        {#each options as option}
            <button 
                class="pill-btn {activeId === option.id ? 'active' : ''}"
                on:click={() => activeId = option.id}
            >
                {option.label}
            </button>
        {/each}
    </div>
</div>
