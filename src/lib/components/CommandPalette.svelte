<script lang="ts">
  import { Command, CornerDownLeft } from "@lucide/svelte";
  import { store, normalize, PROFILES } from "#lib/store.svelte.ts";
  import { theme, THEMES } from "#lib/theme.svelte.ts";
  import { isTauri } from "#lib/api.ts";
  import { i18n, LANGS, t, tn } from "#lib/i18n.svelte.ts";
  import { updates } from "#lib/updates.svelte.ts";
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
      ? [{ id: "a:plan", group: t("palette.group.actions"), label: tn("palette.review", store.pendingCount), run: () => (store.planOpen = true) }]
      : []),
    { id: "v:tweaks", group: t("palette.group.goto"), label: t("nav.all"), run: () => ((store.view = "tweaks"), (store.category = null)) },
    { id: "v:apps", group: t("palette.group.goto"), label: t("palette.apps"), run: () => ((store.view = "apps"), (store.query = "")) },
    { id: "v:disk", group: t("palette.group.goto"), label: t("palette.disk"), run: () => ((store.view = "disk"), (store.query = "")) },
    { id: "v:bloat", group: t("palette.group.goto"), label: t("nav.bloat"), run: () => (store.view = "bloat") },
    { id: "v:versions", group: t("palette.group.goto"), label: t("nav.versions"), run: () => (store.view = "versions") },
    { id: "v:network", group: t("palette.group.goto"), label: t("palette.network"), run: () => (store.view = "network") },
    { id: "v:more", group: t("palette.group.goto"), label: t("nav.more"), run: () => (store.view = "more") },
    { id: "v:history", group: t("palette.group.goto"), label: t("nav.history"), run: () => (store.view = "history") },
    ...(store.overview?.categories ?? []).map((c) => ({
      id: "c:" + c.id, group: t("palette.group.goto"), label: c.title, hint: t("palette.category"),
      run: () => ((store.view = "tweaks"), (store.category = c.id), (store.query = "")),
    })),
    ...THEMES.filter((th) => th.id !== theme.current).map((th) => ({
      id: "th:" + th.id, group: t("palette.group.actions"), label: t("palette.theme", { theme: t(th.title) }), hint: t(th.hint),
      run: () => theme.set(th.id),
    })),
    ...LANGS.filter((l) => l.code !== i18n.lang).map((l) => ({
      id: "lang:" + l.code, group: t("palette.group.language"), label: l.name, hint: l.english,
      run: () => i18n.set(l.code),
    })),
    ...(isTauri
      ? [{
          id: "th:glass", group: t("palette.group.actions"),
          label: theme.glass ? t("palette.glass_off") : t("palette.glass_on"),
          hint: t("palette.glass_hint"),
          run: () => theme.setGlass(!theme.glass),
        }]
      : []),
    { id: "a:update", group: t("palette.group.actions"), label: t("update.check"), run: () => updates.check(false) },
    ...(updates.info
      ? [{ id: "a:install-update", group: t("palette.group.actions"), label: t("update.available", { version: updates.info.version }), hint: t("update.install"), run: () => updates.install() }]
      : []),
    ...PROFILES.map((p) => ({
      id: "p:" + p.id, group: t("palette.group.profiles"), label: t("palette.profile", { profile: t(p.title) }), hint: t(p.hint),
      run: () => store.stageMany(p.ids, true),
    })),
  ]);

  const items = $derived.by<Item[]>(() => {
    const n = normalize(q);
    const tweaks: Item[] = store.tweaks
      .filter((tw) => !store.disabledReason(tw))
      .flatMap((tw) =>
        tw.choices.length
          ? tw.choices.map((c) => ({
              id: `t:${tw.id}:${c.id}`, group: "Tweaks", label: `${tw.title}: ${c.label}`, hint: tw.description,
              state: store.shownChoice(tw) === c.id,
              run: () => store.choose(tw, c.id),
            }))
          : [{
              id: "t:" + tw.id, group: "Tweaks", label: tw.title, hint: tw.description, state: store.shown(tw),
              run: () => store.toggle(tw),
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
    aria-label={t("palette.title")}
  >
    <div class="flex items-center gap-3 border-b border-line px-4">
      <Command size={15} class="text-faint" />
      <input
        bind:this={input}
        bind:value={q}
        onkeydown={onkey}
        placeholder={t("palette.placeholder")}
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
        <li class="px-3 py-6 text-center text-[13px] text-mute">{t("palette.empty", { q })}</li>
      {/each}
    </ul>
    <div class="flex gap-4 border-t border-line px-4 py-2 text-[11px] text-faint">
      <span>↑↓ {t("palette.nav")}</span><span>↵ {t("palette.run")}</span><span>Esc {t("common.close_lower")}</span>
    </div>
  </div>
{/if}
