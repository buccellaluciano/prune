<script lang="ts">
  import { Check, LoaderCircle, PackageX, TriangleAlert } from "@lucide/svelte";
  import { api, type BloatApp, type Risk } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import { t, tn } from "#lib/i18n.svelte.ts";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";

  let apps = $state<BloatApp[] | null>(null);
  let error = $state<string | null>(null);
  let selected = $state<Record<string, boolean>>({});
  let deprovision = $state(true);
  let confirming = $state(false);
  let working = $state(false);

  const tiers = $derived<{ id: Risk; title: string; hint: string; cls: string }[]>([
    { id: "safe", title: t("bloat.tier.safe"), hint: t("bloat.tier.safe_hint"), cls: "" },
    { id: "moderate", title: t("bloat.tier.moderate"), hint: t("bloat.tier.moderate_hint"), cls: "text-moderate" },
    { id: "advanced", title: t("bloat.tier.advanced"), hint: t("bloat.tier.advanced_hint"), cls: "text-advanced" },
  ]);

  const chosen = $derived(Object.keys(selected).filter((k) => selected[k]));
  const elevated = $derived(store.overview?.system.elevated ?? false);
  const foreign = $derived(store.overview?.system.foreign_user ?? null);

  async function load() {
    apps = null;
    error = null;
    try {
      apps = await api.listBloat();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);

  function toggle(id: string) {
    selected = { ...selected, [id]: !selected[id] };
    confirming = false;
  }

  function toggleTier(list: BloatApp[]) {
    const all = list.every((a) => selected[a.id]);
    const copy = { ...selected };
    for (const a of list) copy[a.id] = !all;
    selected = copy;
    confirming = false;
  }

  function clear() {
    selected = {};
    confirming = false;
  }

  async function remove() {
    working = true;
    try {
      const res = await api.removeBloat(chosen, deprovision && elevated);
      const ok = res.filter((r) => r.ok).length;
      if (ok) store.toast("ok", tn("bloat.removed", ok));
      for (const r of res.filter((r) => !r.ok)) store.toast("error", `${r.id}: ${r.error}`);
      clear();
      await load();
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      working = false;
    }
  }
</script>

{#snippet checkbox(on: boolean)}
  <span
    class="grid size-4 shrink-0 place-items-center rounded-[4px] border transition-colors
      {on ? 'border-accent bg-accent text-bg' : 'border-[#3a3935] group-hover:border-mute'}"
    aria-hidden="true"
  >
    {#if on}<Check size={11} strokeWidth={3} />{/if}
  </span>
{/snippet}

<div class="mx-auto max-w-4xl">
  <header class="mb-6">
    <h1 class="font-display text-[20px] font-semibold">{t("nav.bloat")}</h1>
    <p class="mt-1 max-w-2xl text-[13px] text-mute">{t("bloat.subtitle")}</p>
  </header>

  {#if error}
    <p class="rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{error}</p>
  {:else if !apps}
    <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={15} class="animate-spin" /> {t("bloat.loading")}</div>
  {:else if apps.length === 0}
    <div class="rounded-lg border border-line bg-card p-10 text-center">
      <PackageX size={26} strokeWidth={1.6} class="mx-auto text-faint" />
      <p class="mt-3 text-[13.5px] text-mute">{t("bloat.empty")}</p>
    </div>
  {:else}
    {#each tiers as tier (tier.id)}
      {@const list = apps.filter((a) => a.tier === tier.id)}
      {#if list.length}
        {@const all = list.every((a) => selected[a.id])}
        <section class="mb-8">
          <div class="mb-2 flex items-baseline gap-2">
            <h2 class="section-title text-[13px] font-medium {tier.cls}">{tier.title}</h2>
            <span class="font-mono text-[11px] text-faint">{list.length}</span>
            <span class="text-[12px] text-faint">· {tier.hint}</span>
            <button class="ml-auto text-[12px] text-mute hover:text-ink" onclick={() => toggleTier(list)}>
              {all ? t("bloat.unselect_all") : t("bloat.select_all")}
            </button>
          </div>
          <div class="divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
            {#each list as app (app.id)}
              {@const on = !!selected[app.id]}
              <button
                role="checkbox"
                aria-checked={on}
                class="group relative flex w-full items-start gap-3 px-4 py-2.5 text-left transition-colors hover:bg-card-hi {on ? 'bg-card-hi' : ''}"
                onclick={() => toggle(app.id)}
              >
                {#if on}<span class="absolute inset-y-2 left-0 w-0.5 rounded-full bg-accent" aria-hidden="true"></span>{/if}
                <span class="mt-0.5">{@render checkbox(on)}</span>
                <span class="min-w-0 flex-1">
                  <span class="flex items-baseline gap-2">
                    <span class="truncate text-[13.5px] font-medium">{app.title}</span>
                    <span class="truncate font-mono text-[10.5px] text-faint">{app.id}</span>
                  </span>
                  {#if app.note}
                    <span class="mt-0.5 flex gap-1.5 text-[12px] leading-relaxed text-mute">
                      {#if app.tier === "advanced"}<TriangleAlert size={12} class="mt-[3px] shrink-0 text-advanced" />{/if}
                      {app.note}
                    </span>
                  {/if}
                </span>
                <span class="shrink-0 pt-0.5 font-mono text-[11px] text-faint">{app.packages[0]?.version}</span>
              </button>
            {/each}
          </div>
        </section>
      {/if}
    {/each}
  {/if}
</div>

<!-- Barra de acción: solo aparece cuando hay algo seleccionado. -->
{#if chosen.length > 0}
  <div
    transition:fly={{ y: 24, duration: 180 }}
    class="pointer-events-auto absolute bottom-5 left-1/2 z-20 flex -translate-x-1/2 items-center gap-3 rounded-lg border border-[#3a3935] bg-panel py-1.5 pr-1.5 pl-3.5 whitespace-nowrap shadow-lg shadow-black/50"
  >
    {#if confirming}
      <span class="text-[13px] text-moderate">
        {tn(deprovision && elevated ? "bloat.confirm_deprovision" : "bloat.confirm", chosen.length)}
      </span>
      <button class="rounded-md px-2.5 py-1 text-[13px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => (confirming = false)}>
        {t("common.cancel")}
      </button>
      <button
        class="inline-flex items-center gap-2 rounded-md border border-advanced/60 px-3 py-1 text-[13px] font-medium text-advanced hover:bg-advanced/10 disabled:opacity-50"
        disabled={working}
        onclick={remove}
      >
        {#if working}<LoaderCircle size={13} class="animate-spin" />{/if} {t("bloat.yes_remove")}
      </button>
    {:else}
      <span class="text-[13px] text-mute">
        <span class="font-mono text-ink">{chosen.length}</span> {tn("bloat.selected", chosen.length)}
      </span>
      {#if elevated}
        <button
          role="checkbox"
          aria-checked={deprovision}
          class="group flex items-center gap-2 rounded-md px-2 py-1 text-[12.5px] text-mute hover:bg-card-hi hover:text-ink"
          title={t("bloat.deprovision_hint")}
          onclick={() => (deprovision = !deprovision)}
        >
          {@render checkbox(deprovision)} {t("bloat.deprovision")}
        </button>
      {/if}
      <button class="rounded-md px-2.5 py-1 text-[13px] text-mute hover:bg-card-hi hover:text-ink" onclick={clear}>{t("pending.discard")}</button>
      <button
        class="accent-grad rounded-md px-3 py-1 text-[13px] font-medium disabled:opacity-40"
        disabled={!!foreign}
        title={foreign ? t("common.foreign_admin") : undefined}
        onclick={() => (confirming = true)}
      >
        {t("bloat.remove")}
      </button>
    {/if}
  </div>
{/if}
