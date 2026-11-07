<script lang="ts">
  import { AppWindow, Check, HardDrive, History, PackageX, ShieldCheck, ShieldOff, SlidersHorizontal } from "@lucide/svelte";
  import Logo from "./Logo.svelte";
  import { isTauri } from "#lib/api.ts";
  import { disk } from "#lib/disk.svelte.ts";
  import { apps } from "#lib/apps.svelte.ts";
  import { store, PROFILES } from "#lib/store.svelte.ts";
  import CategoryIcon from "./CategoryIcon.svelte";
  import { theme, THEMES } from "#lib/theme.svelte.ts";

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

<aside class="sidebar flex h-full w-60 shrink-0 flex-col border-r border-line bg-panel">
  <div class="flex items-center gap-2.5 px-4 pt-4 pb-4">
    <Logo size={28} />
    <div class="min-w-0">
      <div class="text-[13.5px] leading-tight font-semibold">Prune</div>
      {#if store.overview}
        <div class="font-mono text-[10.5px] text-faint">
          {store.overview.system.display_version} · {store.overview.system.build}.{store.overview.system.ubr}
        </div>
      {/if}
    </div>
  </div>

  <!-- Progreso global -->
  <div class="mx-4 mb-4">
    <div class="flex items-baseline justify-between text-[11.5px] text-mute">
      <span>Optimizado</span><span class="font-mono text-[11px] text-ink">{applied}/{store.tweaks.length}</span>
    </div>
    <div class="mt-1.5 h-[3px] overflow-hidden rounded-full bg-line">
      <div class="progress-fill accent-grad h-full rounded-full transition-all duration-700" style="width:{pct}%"></div>
    </div>
  </div>

  <nav class="flex-1 overflow-y-auto px-2.5 pb-3 text-[13px]">
    <button
      class="nav {store.view === 'tweaks' && !store.category ? 'active' : ''}"
      onclick={() => goCategory(null)}
    >
      <SlidersHorizontal size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">Todos los tweaks</span>
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
      <AppWindow size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">Programas</span>
      {#if apps.busy}<span class="size-1.5 animate-pulse rounded-full bg-accent" title="winget trabajando"></span>
      {:else if apps.upgradable.length}<span class="font-mono text-[11px] text-moderate" title="Actualizaciones disponibles">{apps.upgradable.length}</span>{/if}
    </button>
    <button class="nav {store.view === 'disk' ? 'active' : ''}" onclick={() => ((store.view = "disk"), (store.query = ""))}>
      <HardDrive size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">Disco</span>
      {#if disk.status?.running}<span class="size-1.5 animate-pulse rounded-full bg-accent" title="Analizando"></span>{/if}
    </button>
    <button class="nav {store.view === 'bloat' ? 'active' : ''}" onclick={() => (store.view = "bloat")}>
      <PackageX size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">Bloatware</span>
    </button>
    <button class="nav {store.view === 'history' ? 'active' : ''}" onclick={() => (store.view = "history")}>
      <History size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">Historial</span>
    </button>

    <div class="mt-5 mb-1 px-2.5 text-[11.5px] text-faint">Perfiles</div>
    {#each PROFILES as p (p.id)}
      <button
        class="group w-full rounded-md px-2.5 py-1.5 text-left hover:bg-card-hi"
        onclick={() => {
          store.stageMany(p.ids, true);
          store.view = "tweaks";
          store.toast("info", `Perfil "${p.title}" marcado. Revisá el plan antes de aplicar.`);
        }}
      >
        <div class="text-[13px] text-ink">{p.title}</div>
        <div class="text-[11px] text-faint">{p.hint}</div>
      </button>
    {/each}

    <div class="mt-5 mb-1.5 px-2.5 text-[11.5px] text-faint">Tema</div>
    <div class="theme-picker mx-2.5 flex gap-1 rounded-md border border-line p-0.5" role="radiogroup" aria-label="Tema">
      {#each THEMES as t (t.id)}
        <button
          role="radio"
          aria-checked={theme.current === t.id}
          title={t.hint}
          class="flex-1 rounded-[4px] px-2 py-1 text-[12px] {theme.current === t.id ? 'bg-card-hi font-medium text-ink' : 'text-mute hover:text-ink'}"
          onclick={() => theme.set(t.id)}
        >
          {t.title}
        </button>
      {/each}
    </div>
    <button
      aria-pressed={theme.glass}
      class="glass-toggle group mx-2.5 mt-1.5 flex w-[calc(100%-1.25rem)] items-start gap-2 rounded-md px-1.5 py-1 text-left hover:bg-card-hi disabled:opacity-50"
      disabled={!isTauri}
      title={isTauri ? "Desenfoca el escritorio detrás de la ventana, como el vidrio de Windows 7" : "Solo funciona en la app instalada"}
      onclick={() => theme.setGlass(!theme.glass)}
    >
      <span
        class="mt-0.5 grid size-3.5 shrink-0 place-items-center rounded-[3px] border transition-colors
          {theme.glass ? 'border-accent bg-accent text-bg' : 'border-[#3a3935] group-hover:border-mute'}"
        aria-hidden="true"
      >
        {#if theme.glass}<Check size={10} strokeWidth={3} />{/if}
      </span>
      <span class="min-w-0">
        <span class="block text-[12px] text-ink">Transparencia real</span>
        <span class="block text-[11px] text-faint">Vidrio sobre el escritorio. Puede trabar al mover la ventana.</span>
      </span>
    </button>
  </nav>

  {#if store.user}
    {@const u = store.user}
    {@const elevated = store.overview?.system.elevated ?? false}
    <div class="flex items-center gap-3 border-t border-line px-4 py-3">
      {#if u.picture}
        <img src={u.picture} alt="" class="size-8 shrink-0 rounded-full object-cover ring-1 ring-line" />
      {:else}
        <div class="avatar grid size-8 shrink-0 place-items-center rounded-full border border-line bg-card-hi text-[13px] font-medium text-ink uppercase" aria-hidden="true">
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
    border-radius: 5px;
    padding: 0.35rem 0.6rem;
    color: var(--color-mute);
    transition: background 100ms, color 100ms;
  }
  .nav:hover {
    background: var(--color-card-hi);
    color: var(--color-ink);
  }
  .nav.active {
    background: var(--color-card-hi);
    color: var(--color-ink);
  }
</style>
