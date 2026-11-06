<script lang="ts">
  import { ArrowRight, LoaderCircle, X } from "@lucide/svelte";
  import { api, type PlannedChange } from "$lib/api";
  import { store } from "$lib/store.svelte";
  import { fade, fly } from "svelte/transition";

  let plan = $state<PlannedChange[] | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    if (!store.planOpen) return;
    plan = null;
    error = null;
    api.plan(store.desired()).then((p) => (plan = p)).catch((e) => (error = String(e)));
  });

  const grouped = $derived.by(() => {
    const g = new Map<string, PlannedChange[]>();
    for (const c of plan ?? []) g.set(c.tweak_id, [...(g.get(c.tweak_id) ?? []), c]);
    return [...g.entries()];
  });

  const title = (id: string) => store.tweaks.find((t) => t.id === id)?.title ?? id;

  function verb(id: string): { text: string; cls: string } {
    const p = store.pending[id];
    const t = store.tweaks.find((t) => t.id === id);
    if (typeof p === "string" && t) return { text: `→ ${store.choiceLabel(t, p)}`, cls: "bg-accent/20 text-[#b9b0ff]" };
    return p ? { text: "aplicar", cls: "bg-safe/15 text-safe" } : { text: "revertir", cls: "bg-moderate/15 text-moderate" };
  }
</script>

{#if store.planOpen}
  <div class="fixed inset-0 z-30 bg-black/50" transition:fade={{ duration: 150 }} onclick={() => (store.planOpen = false)} role="presentation"></div>
  <section
    class="fixed top-0 right-0 bottom-0 z-40 flex w-[min(560px,92vw)] flex-col border-l border-line bg-panel shadow-2xl"
    transition:fly={{ x: 560, duration: 240, opacity: 1 }}
    aria-label="Plan de cambios"
  >
    <header class="flex items-center gap-3 border-b border-line px-5 py-4">
      <div class="flex-1">
        <h2 class="font-display text-[16px] font-semibold">Plan de cambios</h2>
        <p class="text-[12px] text-mute">Esto es exactamente lo que se va a escribir en el registro. Todo queda en el historial para deshacer.</p>
      </div>
      <button class="rounded-lg p-1.5 text-mute hover:bg-card-hi hover:text-ink" onclick={() => (store.planOpen = false)} aria-label="Cerrar">
        <X size={18} />
      </button>
    </header>

    <div class="flex-1 space-y-3 overflow-y-auto px-5 py-4">
      {#if error}
        <p class="rounded-lg bg-advanced/10 p-3 text-[13px] text-advanced">{error}</p>
      {:else if !plan}
        <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> Calculando…</div>
      {:else}
        {#each grouped as [id, ops] (id)}
          <div class="rounded-xl border border-line bg-card p-3">
            <div class="mb-2 flex items-center gap-2 text-[13px]">
              <span class="font-medium">{title(id)}</span>
              <span class="rounded-md px-1.5 py-0.5 text-[11px] {verb(id).cls}">{verb(id).text}</span>
            </div>
            {#each ops as c}
              <div class="mt-1.5 rounded-lg bg-bg/60 p-2 font-mono text-[10.5px] {c.noop ? 'opacity-50' : ''}">
                <div class="break-all text-faint">{c.key}</div>
                <div class="mt-0.5 flex flex-wrap items-center gap-2">
                  <span class="text-ink">{c.name}</span>
                  <span class="text-advanced/90 line-through decoration-advanced/50">{c.from}</span>
                  <ArrowRight size={11} class="text-faint" />
                  <span class="text-safe">{c.to}</span>
                  {#if c.noop}<span class="text-faint">(sin cambios)</span>{/if}
                </div>
              </div>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

    <footer class="flex items-center gap-2 border-t border-line px-5 py-4">
      <span class="flex-1 text-[12px] text-mute">{store.pendingCount} tweaks · {plan?.filter((c) => !c.noop).length ?? "…"} valores</span>
      <button class="rounded-lg px-3 py-2 text-[13px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => (store.planOpen = false)}>Volver</button>
      <button
        class="accent-grad inline-flex items-center gap-2 rounded-xl px-4 py-2 text-[13px] font-medium text-white disabled:opacity-50"
        disabled={!plan || store.busy}
        onclick={() => store.applyPending()}
      >
        {#if store.busy}<LoaderCircle size={14} class="animate-spin" />{/if}
        Aplicar cambios
      </button>
    </footer>
  </section>
{/if}
