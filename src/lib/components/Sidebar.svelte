<script lang="ts">
  import { AppWindow, History, PackageX, ShieldCheck, ShieldOff, SlidersHorizontal, Wand2 } from "@lucide/svelte";
  import { apps } from "$lib/apps.svelte";
  import { store, PROFILES } from "$lib/store.svelte";
  import CategoryIcon from "./CategoryIcon.svelte";

  const counts = $derived.by(() => {
    const c: Record<string, { on: number; total: number }> = {};
    for (const t of store.tweaks) {
      c[t.category] ??= { on: 0, total: 0 };
      c[t.category].total++;
      if (t.state === "applied") c[t.category].on++;
    }
    return c;
  });

  const applied = $derived(store.tweaks.filter((t) => t.state === "applied").length);
  const pct = $derived(store.tweaks.length ? Math.round((applied / store.tweaks.length) * 100) : 0);

  function initials(name: string) {
    const parts = name.trim().split(/\s+/);
    return (parts.length > 1 ? parts[0][0] + parts[parts.length - 1][0] : name.slice(0, 1)) || "?";
  }

  function goCategory(id: string | null) {
    store.view = "tweaks";
    store.category = id;
    store.query = "";
  }
</script>

<aside class="flex h-full w-64 shrink-0 flex-col border-r border-line bg-panel/80">
  <div class="flex items-center gap-3 px-5 pt-5 pb-4">
    <div class="accent-grad grid size-9 place-items-center rounded-xl shadow-lg shadow-accent/30">
      <Wand2 size={18} color="white" />
    </div>
    <div>
      <div class="font-display text-[15px] font-semibold tracking-tight">Debloater</div>
      {#if store.overview}
        <div class="text-[11px] text-mute">
          {store.overview.system.display_version} · build {store.overview.system.build}.{store.overview.system.ubr}
        </div>
      {/if}
    </div>
  </div>

  <!-- Progreso global -->
  <div class="mx-4 mb-3 rounded-xl border border-line bg-card px-3 py-2.5">
    <div class="flex items-baseline justify-between text-[11px] text-mute">
      <span>Optimizado</span><span class="font-mono text-ink">{applied}/{store.tweaks.length}</span>
    </div>
    <div class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-line">
      <div class="accent-grad h-full rounded-full transition-all duration-700" style="width:{pct}%"></div>
    </div>
  </div>

  <nav class="flex-1 overflow-y-auto px-2.5 pb-3 text-[13px]">
    <button
      class="nav {store.view === 'tweaks' && !store.category ? 'active' : ''}"
      onclick={() => goCategory(null)}
    >
      <SlidersHorizontal size={16} strokeWidth={1.8} /> <span class="flex-1 text-left">Todos los tweaks</span>
    </button>

    {#each store.overview?.categories ?? [] as c (c.id)}
      <button
        class="nav pl-5 {store.view === 'tweaks' && store.category === c.id ? 'active' : ''}"
        onclick={() => goCategory(c.id)}
      >
        <CategoryIcon name={c.icon} />
        <span class="flex-1 truncate text-left">{c.title}</span>
        <span class="font-mono text-[11px] text-faint">{counts[c.id]?.on ?? 0}/{counts[c.id]?.total ?? 0}</span>
      </button>
    {/each}

    <div class="my-2 border-t border-line"></div>

    <button class="nav {store.view === 'apps' ? 'active' : ''}" onclick={() => ((store.view = "apps"), (store.query = ""))}>
      <AppWindow size={16} strokeWidth={1.8} /> <span class="flex-1 text-left">Programas</span>
      {#if apps.running}<span class="size-2 animate-pulse rounded-full bg-accent" title="winget trabajando"></span>
      {:else if apps.upgradable.length}<span class="rounded-md bg-moderate/15 px-1.5 font-mono text-[10.5px] text-moderate">{apps.upgradable.length}</span>{/if}
    </button>
    <button class="nav {store.view === 'bloat' ? 'active' : ''}" onclick={() => (store.view = "bloat")}>
      <PackageX size={16} strokeWidth={1.8} /> <span class="flex-1 text-left">Bloatware</span>
    </button>
    <button class="nav {store.view === 'history' ? 'active' : ''}" onclick={() => (store.view = "history")}>
      <History size={16} strokeWidth={1.8} /> <span class="flex-1 text-left">Historial</span>
    </button>

    <div class="mt-4 mb-1.5 px-2.5 text-[11px] font-medium tracking-wider text-faint uppercase">Perfiles</div>
    {#each PROFILES as p (p.id)}
      <button
        class="group w-full rounded-lg px-2.5 py-2 text-left hover:bg-card-hi"
        onclick={() => {
          store.stageMany(p.ids, true);
          store.view = "tweaks";
          store.toast("info", `Perfil "${p.title}" marcado. Revisá el plan antes de aplicar.`);
        }}
      >
        <div class="text-[13px] text-ink">{p.title}</div>
        <div class="text-[11px] text-mute">{p.hint}</div>
      </button>
    {/each}
  </nav>

  {#if store.user}
    {@const u = store.user}
    {@const elevated = store.overview?.system.elevated ?? false}
    <div class="flex items-center gap-3 border-t border-line px-4 py-3">
      {#if u.picture}
        <img src={u.picture} alt="" class="size-9 shrink-0 rounded-full object-cover ring-1 ring-line" />
      {:else}
        <div class="accent-grad grid size-9 shrink-0 place-items-center rounded-full text-[14px] font-semibold text-white uppercase" aria-hidden="true">
          {initials(u.name || u.username)}
        </div>
      {/if}
      <div class="min-w-0 flex-1">
        <div class="truncate text-[13px] font-medium" title={u.name}>{u.name || u.username}</div>
        <div
          class="flex items-center gap-1 text-[11px] {elevated ? 'text-safe' : 'text-faint'}"
          title={elevated ? "La app corre con permisos de administrador" : "La app corre sin permisos de administrador"}
        >
          {#if elevated}<ShieldCheck size={11} /> Administrador{:else}<ShieldOff size={11} /> Sin admin{/if}
          {#if u.name && u.name !== u.username}<span class="truncate text-faint">· {u.username}</span>{/if}
        </div>
      </div>
    </div>
  {/if}
</aside>

<style>
  .nav {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.6rem;
    border-radius: 0.6rem;
    padding: 0.45rem 0.65rem;
    color: var(--color-mute);
    transition: background 120ms, color 120ms;
  }
  .nav:hover {
    background: var(--color-card-hi);
    color: var(--color-ink);
  }
  .nav.active {
    background: rgb(124 108 255 / 0.14);
    color: var(--color-ink);
    box-shadow: inset 2px 0 0 var(--color-accent);
  }
</style>
