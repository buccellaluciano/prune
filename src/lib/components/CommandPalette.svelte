<script lang="ts">
  import { Command, CornerDownLeft } from "@lucide/svelte";
  import { store, normalize, PROFILES } from "$lib/store.svelte";
  import { theme, THEMES } from "$lib/theme.svelte";
  import { fade, scale } from "svelte/transition";
  import { tick } from "svelte";

  interface Item {
    id: string;
    group: string;
    label: string;
    hint?: string;
    state?: boolean;
    run: () => void;
  }

  let q = $state("");
  let sel = $state(0);
  let input = $state<HTMLInputElement>();

  const actions = $derived<Item[]>([
    ...(store.pendingCount
      ? [{ id: "a:plan", group: "Acciones", label: `Revisar y aplicar ${store.pendingCount} cambios`, run: () => (store.planOpen = true) }]
      : []),
    { id: "v:tweaks", group: "Ir a", label: "Todos los tweaks", run: () => ((store.view = "tweaks"), (store.category = null)) },
    { id: "v:apps", group: "Ir a", label: "Programas (winget)", run: () => ((store.view = "apps"), (store.query = "")) },
    { id: "v:disk", group: "Ir a", label: "Disco (uso de espacio)", run: () => ((store.view = "disk"), (store.query = "")) },
    { id: "v:bloat", group: "Ir a", label: "Bloatware", run: () => (store.view = "bloat") },
    { id: "v:history", group: "Ir a", label: "Historial", run: () => (store.view = "history") },
    ...(store.overview?.categories ?? []).map((c) => ({
      id: "c:" + c.id, group: "Ir a", label: c.title, hint: "categoría",
      run: () => ((store.view = "tweaks"), (store.category = c.id), (store.query = "")),
    })),
    ...THEMES.filter((t) => t.id !== theme.current).map((t) => ({
      id: "th:" + t.id, group: "Acciones", label: `Cambiar al tema ${t.title}`, hint: t.hint,
      run: () => theme.set(t.id),
    })),
    ...PROFILES.map((p) => ({
      id: "p:" + p.id, group: "Perfiles", label: `Marcar perfil: ${p.title}`, hint: p.hint,
      run: () => store.stageMany(p.ids, true),
    })),
  ]);

  const items = $derived.by<Item[]>(() => {
    const n = normalize(q);
    const tweaks: Item[] = store.tweaks
      .filter((t) => !store.disabledReason(t))
      .flatMap((t) =>
        t.choices.length
          ? t.choices.map((c) => ({
              id: `t:${t.id}:${c.id}`, group: "Tweaks", label: `${t.title}: ${c.label}`, hint: t.description,
              state: store.shownChoice(t) === c.id,
              run: () => store.choose(t, c.id),
            }))
          : [{
              id: "t:" + t.id, group: "Tweaks", label: t.title, hint: t.description, state: store.shown(t),
              run: () => store.toggle(t),
            }],
      );
    const all = [...actions, ...tweaks];
    if (!n) return all.slice(0, 40);
    return all.filter((i) => normalize(`${i.label} ${i.hint ?? ""}`).includes(n)).slice(0, 40);
  });

  $effect(() => {
    if (store.paletteOpen) {
      q = "";
      sel = 0;
      tick().then(() => input?.focus());
    }
  });

  $effect(() => {
    void q;
    sel = 0;
  });

  function run(i: Item) {
    i.run();
    // Los toggles dejan la paleta abierta para marcar varios seguidos.
    if (!i.id.startsWith("t:")) store.paletteOpen = false;
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") { sel = Math.min(sel + 1, items.length - 1); e.preventDefault(); scrollSel(); }
    else if (e.key === "ArrowUp") { sel = Math.max(sel - 1, 0); e.preventDefault(); scrollSel(); }
    else if (e.key === "Enter" && items[sel]) { run(items[sel]); e.preventDefault(); }
    else if (e.key === "Escape") store.paletteOpen = false;
  }

  function scrollSel() {
    tick().then(() => document.querySelector(`[data-idx="${sel}"]`)?.scrollIntoView({ block: "nearest" }));
  }
</script>

{#if store.paletteOpen}
  <div class="fixed inset-0 z-50 bg-black/55 backdrop-blur-[2px]" transition:fade={{ duration: 120 }} onclick={() => (store.paletteOpen = false)} role="presentation"></div>
  <div
    class="fixed top-[12vh] left-1/2 z-50 w-[min(640px,92vw)] -translate-x-1/2 overflow-hidden rounded-lg border border-[#3a3935] bg-panel shadow-xl shadow-black/60"
    transition:scale={{ start: 0.97, duration: 140 }}
    role="dialog"
    aria-label="Paleta de comandos"
  >
    <div class="flex items-center gap-3 border-b border-line px-4">
      <Command size={15} class="text-faint" />
      <input
        bind:this={input}
        bind:value={q}
        onkeydown={onkey}
        placeholder="Buscá un tweak, una categoría o una acción…"
        class="h-13 flex-1 bg-transparent text-[14px] outline-none placeholder:text-faint"
      />
      <kbd class="rounded border border-line px-1.5 py-0.5 font-mono text-[10px] text-faint">ESC</kbd>
    </div>
    <ul class="max-h-[52vh] overflow-y-auto p-2">
      {#each items as item, i (item.id)}
        {#if i === 0 || items[i - 1].group !== item.group}
          <li class="px-2.5 pt-2.5 pb-1 text-[11.5px] text-faint">{item.group}</li>
        {/if}
        <li>
          <button
            data-idx={i}
            class="flex w-full items-center gap-3 rounded-md px-2.5 py-1.5 text-left {i === sel ? 'bg-card-hi' : ''}"
            onmouseenter={() => (sel = i)}
            onclick={() => run(item)}
          >
            {#if item.state !== undefined}
              <span class="h-4 w-7 shrink-0 rounded-full p-0.5 transition-colors {item.state ? 'accent-grad' : 'bg-line'}">
                <span class="block size-3 rounded-full bg-white transition-transform {item.state ? 'translate-x-3' : ''}"></span>
              </span>
            {/if}
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[13px]">{item.label}</span>
              {#if item.hint}<span class="block truncate text-[11.5px] text-mute">{item.hint}</span>{/if}
            </span>
            {#if i === sel}<CornerDownLeft size={13} class="text-faint" />{/if}
          </button>
        </li>
      {:else}
        <li class="px-3 py-6 text-center text-[13px] text-mute">Nada coincide con "{q}"</li>
      {/each}
    </ul>
    <div class="flex gap-4 border-t border-line px-4 py-2 text-[11px] text-faint">
      <span>↑↓ navegar</span><span>↵ ejecutar / marcar</span><span>Esc cerrar</span>
    </div>
  </div>
{/if}
