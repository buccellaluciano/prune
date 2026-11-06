<script lang="ts">
  import { Command, LoaderCircle, Search, ShieldAlert, X } from "@lucide/svelte";
  import { isTauri } from "$lib/api";
  import { store } from "$lib/store.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import TweakCard from "$lib/components/TweakCard.svelte";
  import PendingBar from "$lib/components/PendingBar.svelte";
  import PlanSheet from "$lib/components/PlanSheet.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import BloatView from "$lib/components/BloatView.svelte";
  import HistoryView from "$lib/components/HistoryView.svelte";
  import AppsView from "$lib/components/AppsView.svelte";
  import { fly } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { onMount } from "svelte";

  onMount(() => {
    store.load();
    store.loadUser();
  });

  function onkeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      store.paletteOpen = !store.paletteOpen;
    } else if (e.key === "Escape" && store.planOpen) {
      store.planOpen = false;
    }
  }

  const categoryTitle = $derived(
    store.query
      ? `Resultados para "${store.query}"`
      : (store.overview?.categories.find((c) => c.id === store.category)?.title ?? "Todos los tweaks"),
  );

  const grouped = $derived.by(() => {
    const cats = store.overview?.categories ?? [];
    if (store.category && !store.query) return [{ cat: null, items: store.filtered }];
    return cats
      .map((c) => ({ cat: c, items: store.filtered.filter((t) => t.category === c.id) }))
      .filter((g) => g.items.length);
  });

  const toastCls = { ok: "border-safe/40", error: "border-advanced/50", info: "border-accent-2/40" } as const;
</script>

<svelte:window {onkeydown} />

<div class="glow flex h-screen overflow-hidden">
  <Sidebar />

  <main class="relative flex min-w-0 flex-1 flex-col">
    <!-- Barra superior -->
    <div class="flex items-center gap-3 border-b border-line px-6 py-3">
      <div class="relative flex-1">
        <Search size={15} class="absolute top-1/2 left-3 -translate-y-1/2 text-faint" />
        <input
          bind:value={store.query}
          oninput={() => { if (store.view !== "apps") store.view = "tweaks"; }}
          placeholder={store.view === "apps" ? "Filtrar programas…" : "Filtrar tweaks…"}
          class="h-9 w-full max-w-md rounded-xl border border-line bg-card pr-9 pl-9 text-[13px] outline-none placeholder:text-faint focus:border-accent/60"
        />
      </div>
      {#if !isTauri}
        <span class="rounded-md bg-moderate/15 px-2 py-1 text-[11px] text-moderate">modo demo (navegador)</span>
      {/if}
      <button
        class="inline-flex items-center gap-2 rounded-xl border border-line bg-card px-3 py-1.5 text-[12.5px] text-mute hover:text-ink"
        onclick={() => (store.paletteOpen = true)}
      >
        <Command size={13} /> Comandos <kbd class="font-mono text-[10.5px] text-faint">Ctrl K</kbd>
      </button>
    </div>

    {#if store.overview && !store.overview.system.elevated}
      <div class="flex items-center gap-2 border-b border-moderate/20 bg-moderate/8 px-6 py-2 text-[12.5px] text-moderate">
        <ShieldAlert size={15} />
        Sin permisos de administrador: los tweaks de sistema (HKLM) y evitar reinstalaciones de apps quedan deshabilitados.
      </div>
    {/if}

    <div class="flex-1 overflow-y-auto px-6 pt-6 pb-28">
      {#if store.loading && !store.overview}
        <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> Leyendo el estado del sistema…</div>
      {:else if store.loadError}
        <p class="rounded-xl bg-advanced/10 p-4 text-[13px] text-advanced">{store.loadError}</p>
      {:else if store.view === "bloat"}
        <BloatView />
      {:else if store.view === "apps"}
        <AppsView />
      {:else if store.view === "history"}
        <HistoryView />
      {:else}
        <div class="mx-auto max-w-6xl">
          <h1 class="font-display text-[24px] font-semibold tracking-tight">{categoryTitle}</h1>
          <p class="mt-1 mb-6 text-[13px] text-mute">
            Marcá lo que quieras cambiar: nada se aplica hasta que revisás el plan. El estado se lee del registro real.
          </p>

          {#each grouped as g (g.cat?.id ?? "single")}
            {#if g.cat}
              <h2 class="mt-2 mb-3 text-[12px] font-medium tracking-wider text-faint uppercase">{g.cat.title}</h2>
            {/if}
            <div class="mb-8 grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-3">
              {#each g.items as t (t.id)}
                <div animate:flip={{ duration: 200 }}>
                  <TweakCard tweak={t} />
                </div>
              {/each}
            </div>
          {:else}
            <p class="text-[13px] text-mute">Ningún tweak coincide con la búsqueda.</p>
          {/each}
        </div>
      {/if}
    </div>

    <PendingBar />
  </main>
</div>

<PlanSheet />
<CommandPalette />

<!-- Toasts -->
<div class="pointer-events-none fixed right-5 bottom-5 z-[60] flex w-80 flex-col gap-2">
  {#each store.toasts as t (t.id)}
    <div
      transition:fly={{ x: 40, duration: 200 }}
      class="pointer-events-auto flex items-start gap-2 rounded-xl border bg-panel/95 p-3 text-[12.5px] shadow-xl backdrop-blur {toastCls[t.kind]}"
    >
      <span class="flex-1">{t.text}</span>
      <button class="text-faint hover:text-ink" onclick={() => (store.toasts = store.toasts.filter((x) => x.id !== t.id))} aria-label="Cerrar">
        <X size={13} />
      </button>
    </div>
  {/each}
</div>
