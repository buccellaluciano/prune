<script lang="ts">
  import { store } from "$lib/store.svelte";
  import { fly } from "svelte/transition";
</script>

{#if store.pendingCount > 0 && store.view === "tweaks"}
  <div
    transition:fly={{ y: 40, duration: 220 }}
    class="pointer-events-auto absolute bottom-5 left-1/2 z-20 flex whitespace-nowrap -translate-x-1/2 items-center gap-3 rounded-2xl border border-line bg-panel/95 py-2 pr-2 pl-4 shadow-2xl shadow-black/60 backdrop-blur"
  >
    <span class="accent-grad grid size-6 place-items-center rounded-full font-mono text-[12px] font-semibold text-white">
      {store.pendingCount}
    </span>
    <span class="text-[13px] text-mute">
      {store.pendingCount === 1 ? "cambio pendiente" : "cambios pendientes"}
    </span>
    <button class="rounded-lg px-3 py-1.5 text-[13px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => store.clearPending()}>
      Descartar
    </button>
    <button
      class="accent-grad rounded-xl px-4 py-1.5 text-[13px] font-medium text-white shadow-lg shadow-accent/30 hover:brightness-110"
      onclick={() => (store.planOpen = true)}
    >
      Revisar y aplicar
    </button>
  </div>
{/if}
