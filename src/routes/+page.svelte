<script lang="ts">
  import { Command, LoaderCircle, Search, ShieldAlert, X } from "@lucide/svelte";
  import { isTauri } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import Sidebar from "#lib/components/Sidebar.svelte";
  import TweakCard from "#lib/components/TweakCard.svelte";
  import PendingBar from "#lib/components/PendingBar.svelte";
  import PlanSheet from "#lib/components/PlanSheet.svelte";
  import CommandPalette from "#lib/components/CommandPalette.svelte";
  import BloatView from "#lib/components/BloatView.svelte";
  import HistoryView from "#lib/components/HistoryView.svelte";
  import AppsView from "#lib/components/AppsView.svelte";
  import DiskView from "#lib/components/DiskView.svelte";
  import VersionsView from "#lib/components/VersionsView.svelte";
  import InstallSheet from "#lib/components/InstallSheet.svelte";
  import Welcome from "#lib/components/Welcome.svelte";
  import UpdateSheet from "#lib/components/UpdateSheet.svelte";
  import { updates } from "#lib/updates.svelte.ts";
  import { fly } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { onMount } from "svelte";
  import TitleBar from "#lib/components/TitleBar.svelte";
  import ModernTitleBar from "#lib/components/ModernTitleBar.svelte";
  import { theme } from "#lib/theme.svelte.ts";
  import { apps } from "#lib/apps.svelte.ts";
  import { i18n, t } from "#lib/i18n.svelte.ts";

  onMount(() => {
    theme.apply();
    store.loadUser();
    // El idioma va primero: el backend arma el catálogo y los mensajes en ese idioma.
    // Después, en silencio, si hay una versión nueva (un pedido a GitHub; si falla, nada).
    i18n.init().finally(() => store.load().then(() => updates.check(true)));
    // Al cambiarlo, se vuelve a pedir lo que trae textos del catálogo.
    i18n.onChange(() => {
      store.load();
      if (apps.overview) apps.load();
    });
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
      ? t("page.results", { q: store.query })
      : (store.overview?.categories.find((c) => c.id === store.category)?.title ?? t("nav.all")),
  );

  const grouped = $derived.by(() => {
    const cats = store.overview?.categories ?? [];
    if (store.category && !store.query) return [{ cat: null, items: store.filtered }];
    return cats
      .map((c) => ({ cat: c, items: store.filtered.filter((t) => t.category === c.id) }))
      .filter((g) => g.items.length);
  });

  const toastCls = { ok: "border-l-safe", error: "border-l-advanced", info: "border-l-mute" } as const;
</script>

<svelte:window {onkeydown} />

<div class="app-frame flex h-screen flex-col overflow-hidden">
  {#if theme.current === "aero"}<TitleBar />{:else if theme.glass && isTauri}<ModernTitleBar />{/if}
  <div class="flex min-h-0 flex-1">
  <Sidebar />

  <main class="relative flex min-w-0 flex-1 flex-col">
    <!-- Barra superior -->
    <div class="toolbar flex items-center gap-3 border-b border-line px-6 py-2.5">
      <div class="relative flex-1">
        <Search size={14} class="absolute top-1/2 left-2.5 -translate-y-1/2 text-faint" />
        <input
          bind:value={store.query}
          oninput={() => { if (store.view !== "apps") store.view = "tweaks"; }}
          placeholder={store.view === "apps" ? t("page.filter_apps") : t("page.filter_tweaks")}
          class="h-8 w-full max-w-sm rounded-md border border-line bg-transparent pr-3 pl-8 text-[13px] outline-none placeholder:text-faint focus:border-mute"
        />
      </div>
      {#if !isTauri}
        <span class="font-mono text-[11px] text-faint">{t("page.demo")}</span>
      {/if}
      <button
        class="inline-flex items-center gap-2 rounded-md border border-line px-2.5 py-1 text-[12.5px] text-mute hover:border-[#3a3935] hover:text-ink"
        onclick={() => (store.paletteOpen = true)}
      >
        <Command size={13} /> {t("page.commands")} <kbd class="font-mono text-[10.5px] text-faint">Ctrl K</kbd>
      </button>
    </div>

    {#if store.overview?.system.foreign_user}
      <div class="infobar flex items-center gap-2 border-b border-line px-6 py-2 text-[12px] text-mute">
        <ShieldAlert size={14} class="text-advanced" />
        {t("page.foreign", { user: store.overview.system.foreign_user })}
      </div>
    {:else if store.overview && !store.overview.system.elevated}
      <div class="infobar flex items-center gap-2 border-b border-line px-6 py-2 text-[12px] text-mute">
        <ShieldAlert size={14} class="text-moderate" />
        {t("page.not_admin")}
      </div>
    {/if}

    <div class="flex-1 overflow-y-auto px-6 pt-6 pb-28">
      {#if store.loading && !store.overview}
        <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> {t("page.loading")}</div>
      {:else if store.loadError}
        <p class="rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{store.loadError}</p>
      {:else if store.view === "bloat"}
        <!-- Al cambiar de idioma se vuelven a montar: piden sus datos ya traducidos. -->
        {#key i18n.lang}<BloatView />{/key}
      {:else if store.view === "apps"}
        <AppsView />
      {:else if store.view === "disk"}
        <DiskView />
      {:else if store.view === "history"}
        {#key i18n.lang}<HistoryView />{/key}
      {:else if store.view === "versions"}
        <VersionsView />
      {:else}
        <div class="mx-auto max-w-4xl">
          <h1 class="font-display text-[20px] font-semibold">{categoryTitle}</h1>
          <p class="mt-1 mb-7 text-[13px] text-mute">{t("page.subtitle")}</p>

          {#each grouped as g (g.cat?.id ?? "single")}
            {#if g.cat}
              <h2 class="section-title mb-2 flex items-baseline gap-2 text-[13px] font-medium">
                {g.cat.title}
                <span class="font-mono text-[11px] font-normal text-faint">
                  {g.items.filter((t) => t.state === "applied").length}/{g.items.length}
                </span>
              </h2>
            {/if}
            <div class="tweak-list mb-8 divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
              {#each g.items as t (t.id)}
                <div animate:flip={{ duration: 200 }}>
                  <TweakCard tweak={t} />
                </div>
              {/each}
            </div>
          {:else}
            <p class="text-[13px] text-mute">{t("page.no_match")}</p>
          {/each}
        </div>
      {/if}
    </div>

    <PendingBar />
  </main>
  </div>
</div>

<PlanSheet />
<CommandPalette />
<InstallSheet />
<UpdateSheet />
<Welcome />

<!-- Toasts -->
<div class="pointer-events-none fixed right-5 bottom-5 z-[60] flex w-80 flex-col gap-2">
  {#each store.toasts as toast (toast.id)}
    <div
      transition:fly={{ x: 40, duration: 200 }}
      class="toast pointer-events-auto flex items-start gap-2 rounded-md border border-l-2 border-line bg-panel p-3 text-[12.5px] shadow-lg shadow-black/40 {toastCls[toast.kind]}"
    >
      <span class="flex-1">{toast.text}</span>
      <button class="text-faint hover:text-ink" onclick={() => (store.toasts = store.toasts.filter((x) => x.id !== toast.id))} aria-label={t("common.close")}>
        <X size={13} />
      </button>
    </div>
  {/each}
</div>
