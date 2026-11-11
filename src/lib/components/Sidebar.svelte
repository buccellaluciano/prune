<script lang="ts">
  import { AppWindow, Check, Globe, Grid2x2Plus, HardDrive, History, LoaderCircle, PackageX, RefreshCw, ShieldCheck, ShieldOff, SlidersHorizontal, Tag } from "@lucide/svelte";
  import Logo from "./Logo.svelte";
  import { isTauri } from "#lib/api.ts";
  import { disk } from "#lib/disk.svelte.ts";
  import { apps } from "#lib/apps.svelte.ts";
  import { store, PROFILES } from "#lib/store.svelte.ts";
  import CategoryIcon from "./CategoryIcon.svelte";
  import { theme, THEMES } from "#lib/theme.svelte.ts";
  import { initials } from "#lib/format.ts";
  import { i18n, LANGS, t } from "#lib/i18n.svelte.ts";
  import { updates } from "#lib/updates.svelte.ts";
  import UpdateNotice from "./UpdateNotice.svelte";

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
      <div class="text-[13.5px] leading-tight font-semibold">P-Rune</div>
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
      <span>{t("side.optimized")}</span><span class="font-mono text-[11px] text-ink">{applied}/{store.tweaks.length}</span>
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
      <SlidersHorizontal size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">{t("nav.all")}</span>
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
      <AppWindow size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">{t("nav.apps")}</span>
      {#if apps.busy}<span class="size-1.5 animate-pulse rounded-full bg-accent" title={t("side.winget_busy")}></span>
      {:else if apps.upgradable.length}<span class="font-mono text-[11px] text-moderate" title={t("side.updates")}>{apps.upgradable.length}</span>{/if}
    </button>
    <button class="nav {store.view === 'disk' ? 'active' : ''}" onclick={() => ((store.view = "disk"), (store.query = ""))}>
      <HardDrive size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">{t("nav.disk")}</span>
      {#if disk.status?.running}<span class="size-1.5 animate-pulse rounded-full bg-accent" title={t("side.scanning")}></span>{/if}
    </button>
    <button class="nav {store.view === 'network' ? 'active' : ''}" onclick={() => (store.view = "network")}>
      <Globe size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">{t("nav.network")}</span>
    </button>
    <button class="nav {store.view === 'bloat' ? 'active' : ''}" onclick={() => (store.view = "bloat")}>
      <PackageX size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">{t("nav.bloat")}</span>
    </button>
    <button class="nav {store.view === 'history' ? 'active' : ''}" onclick={() => (store.view = "history")}>
      <History size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">{t("nav.history")}</span>
    </button>
    <button class="nav {store.view === 'more' ? 'active' : ''}" onclick={() => (store.view = "more")}>
      <Grid2x2Plus size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">{t("nav.more")}</span>
    </button>
    <button class="nav {store.view === 'versions' ? 'active' : ''}" onclick={() => (store.view = "versions")}>
      <Tag size={15} strokeWidth={1.7} /> <span class="flex-1 text-left">{t("nav.versions")}</span>
      {#if updates.info}<span class="size-1.5 rounded-full bg-accent" title={t("update.available", { version: updates.info.version })}></span>{/if}
    </button>

    <div class="mt-5 mb-1 px-2.5 text-[11.5px] text-faint">{t("side.profiles")}</div>
    {#each PROFILES as p (p.id)}
      <button
        class="group w-full rounded-md px-2.5 py-1.5 text-left hover:bg-card-hi"
        onclick={() => {
          store.stageMany(p.ids, true);
          store.view = "tweaks";
          store.toast("info", t("side.profile_staged", { profile: t(p.title) }));
        }}
      >
        <div class="text-[13px] text-ink">{t(p.title)}</div>
        <div class="text-[11px] text-faint">{t(p.hint)}</div>
      </button>
    {/each}

    <div class="mt-5 mb-1.5 px-2.5 text-[11.5px] text-faint">{t("side.theme")}</div>
    <div class="theme-picker mx-2.5 flex gap-1 rounded-md border border-line p-0.5" role="radiogroup" aria-label={t("side.theme")}>
      {#each THEMES as th (th.id)}
        <button
          role="radio"
          aria-checked={theme.current === th.id}
          title={t(th.hint)}
          class="flex-1 rounded-[4px] px-2 py-1 text-[12px] {theme.current === th.id ? 'bg-card-hi font-medium text-ink' : 'text-mute hover:text-ink'}"
          onclick={() => theme.set(th.id)}
        >
          {t(th.title)}
        </button>
      {/each}
    </div>
    <button
      aria-pressed={theme.glass}
      class="glass-toggle group mx-2.5 mt-1.5 flex w-[calc(100%-1.25rem)] items-start gap-2 rounded-md px-1.5 py-1 text-left hover:bg-card-hi disabled:opacity-50"
      disabled={!isTauri}
      title={isTauri ? t("side.glass_title") : t("side.glass_unavailable")}
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
        <span class="block text-[12px] text-ink">{t("side.glass")}</span>
        <span class="block text-[11px] text-faint">{t("side.glass_hint")}</span>
      </span>
    </button>

    <label class="mt-5 mb-1.5 block px-2.5 text-[11.5px] text-faint" for="lang-select">{t("side.language")}</label>
    <select
      id="lang-select"
      class="lang-select mx-2.5 block w-[calc(100%-1.25rem)] rounded-md border border-line bg-panel px-2 py-1 text-[12px] text-ink outline-none focus:border-mute"
      value={i18n.lang}
      onchange={(e) => i18n.set(e.currentTarget.value as (typeof LANGS)[number]["code"])}
    >
      {#each LANGS as l (l.code)}
        <option value={l.code}>{l.name}</option>
      {/each}
    </select>

    <button
      class="mx-2.5 mt-3 inline-flex items-center gap-1.5 px-0.5 text-[11.5px] text-faint hover:text-ink disabled:opacity-60"
      disabled={updates.checking || updates.installing}
      onclick={() => updates.check(false)}
    >
      {#if updates.checking}<LoaderCircle size={11} class="animate-spin" />{:else}<RefreshCw size={11} />{/if}
      {updates.checking ? t("update.checking") : t("update.check")}
    </button>
  </nav>

  <UpdateNotice />

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
          title={elevated ? t("side.elevated_title") : t("side.not_elevated_title")}
        >
          {#if elevated}<ShieldCheck size={11} /> {t("side.admin")}{:else}<ShieldOff size={11} /> {t("side.no_admin")}{/if}
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
