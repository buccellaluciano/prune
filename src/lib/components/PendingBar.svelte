<script lang="ts">
  import { store } from "#lib/store.svelte.ts";
  import { fly } from "svelte/transition";
</script>

{#if store.pendingCount > 0 && store.view === "tweaks"}
  <div
    transition:fly={{ y: 24, duration: 180 }}
    class="pointer-events-auto absolute bottom-5 left-1/2 z-20 flex -translate-x-1/2 items-center gap-3 rounded-lg border border-[#3a3935] bg-panel py-1.5 pr-1.5 pl-3.5 whitespace-nowrap shadow-lg shadow-black/50"
  >
    <span class="text-[13px] text-mute">
      <span class="font-mono text-ink">{store.pendingCount}</span>
      {store.pendingCount === 1 ? "cambio pendiente" : "cambios pendientes"}
    </span>
    <button class="rounded-md px-2.5 py-1 text-[13px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => store.clearPending()}>
      Descartar
    </button>
    <button class="accent-grad rounded-md px-3 py-1 text-[13px] font-medium" onclick={() => (store.planOpen = true)}>
      Revisar y aplicar
    </button>
  </div>
{/if}
