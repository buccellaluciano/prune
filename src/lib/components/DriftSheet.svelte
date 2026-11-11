<script lang="ts">
  /** Cambios que Windows volvió atrás: elegir cuáles reaplicar o ignorar. */
  import { Check, LoaderCircle, X } from "@lucide/svelte";
  import { drift } from "#lib/safety.svelte.ts";
  import { i18n, t } from "#lib/i18n.svelte.ts";
  import { fade, fly } from "svelte/transition";
  import type { Drift } from "#lib/api.ts";

  let selected = $state<Record<string, boolean>>({});
  let working = $state(false);

  const k = (d: Drift) => `${d.kind}:${d.id}`;
  $effect(() => {
    if (drift.open) selected = Object.fromEntries(drift.visible.map((d) => [k(d), true]));
  });

  const chosen = $derived(drift.visible.filter((d) => selected[k(d)]));
  const tweaks = $derived(drift.visible.filter((d) => d.kind === "tweak"));
  const apps = $derived(drift.visible.filter((d) => d.kind === "bloat"));
  const fmt = $derived(new Intl.DateTimeFormat(i18n.locale, { dateStyle: "medium" }));

  async function reapply() {
    working = true;
    try {
      await drift.reapply(chosen);
    } finally {
      working = false;
    }
  }

  function ignore() {
    drift.ignore(chosen.length ? chosen : drift.visible);
    drift.open = false;
  }
</script>

{#snippet row(d: Drift)}
  <button
    role="checkbox"
    aria-checked={!!selected[k(d)]}
    class="flex w-full items-start gap-2.5 px-3 py-2.5 text-left hover:bg-card-hi"
    onclick={() => (selected = { ...selected, [k(d)]: !selected[k(d)] })}
  >
    <span class="mt-0.5 flex size-3.5 shrink-0 items-center justify-center rounded-[4px] border {selected[k(d)] ? 'border-accent bg-accent text-bg' : 'border-[#3a3935]'}">
      {#if selected[k(d)]}<Check size={10} strokeWidth={3} />{/if}
    </span>
    <span class="min-w-0 flex-1">
      <span class="block text-[13px]">{d.title}</span>
      <span class="block text-[11.5px] text-faint">{t("drift.applied_on", { date: fmt.format(new Date(d.at * 1000)) })}</span>
    </span>
  </button>
{/snippet}

{#if drift.open}
  <div class="fixed inset-0 z-30 bg-black/50" transition:fade={{ duration: 150 }} onclick={() => (drift.open = false)} role="presentation"></div>
  <section
    class="fixed top-0 right-0 bottom-0 z-40 flex w-[min(520px,92vw)] flex-col border-l border-line bg-panel shadow-xl shadow-black/50"
    transition:fly={{ x: 520, duration: 240, opacity: 1 }}
    aria-label={t("drift.title")}
  >
    <header class="flex items-center gap-3 border-b border-line px-5 py-4">
      <div class="flex-1">
        <h2 class="font-display text-[16px] font-semibold">{t("drift.title")}</h2>
        <p class="text-[12px] text-mute">{t("drift.subtitle")}</p>
      </div>
      <button class="rounded-lg p-1.5 text-mute hover:bg-card-hi hover:text-ink" onclick={() => (drift.open = false)} aria-label={t("common.close")}>
        <X size={18} />
      </button>
    </header>

    <div class="flex-1 space-y-5 overflow-y-auto px-5 py-4">
      {#if tweaks.length}
        <div>
          <h3 class="mb-1.5 text-[12px] font-medium text-mute">{t("drift.tweaks")}</h3>
          <div class="divide-y divide-line overflow-hidden rounded-md border border-line bg-card">
            {#each tweaks as d (k(d))}{@render row(d)}{/each}
          </div>
        </div>
      {/if}
      {#if apps.length}
        <div>
          <h3 class="mb-1.5 text-[12px] font-medium text-mute">{t("drift.apps")}</h3>
          <div class="divide-y divide-line overflow-hidden rounded-md border border-line bg-card">
            {#each apps as d (k(d))}{@render row(d)}{/each}
          </div>
          <p class="mt-1.5 text-[11.5px] text-faint">{t("drift.apps_hint")}</p>
        </div>
      {/if}
    </div>

    <footer class="flex items-center gap-2 border-t border-line px-5 py-4">
      <button class="flex-1 text-left text-[12px] text-mute hover:text-ink" title={t("drift.ignore_hint")} onclick={ignore}>
        {t("drift.ignore")}
      </button>
      <button
        class="accent-grad inline-flex items-center gap-2 rounded-md px-3.5 py-1.5 text-[13px] font-medium disabled:opacity-50"
        disabled={!chosen.length || working}
        onclick={reapply}
      >
        {#if working}<LoaderCircle size={14} class="animate-spin" />{/if}
        {t("drift.reapply")}
      </button>
    </footer>
  </section>
{/if}
