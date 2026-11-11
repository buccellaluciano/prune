<script lang="ts">
  import { ArrowRight, LoaderCircle, X } from "@lucide/svelte";
  import { api, type PlannedChange } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import { fade, fly } from "svelte/transition";
  import { restore } from "#lib/safety.svelte.ts";
  import RestoreOption from "./RestoreOption.svelte";

  let plan = $state<PlannedChange[] | null>(null);
  let error = $state<string | null>(null);

  /** Punto de restauración antes de aplicar: por defecto en cambios grandes o con riesgo. */
  let withRestore = $state(false);

  $effect(() => {
    if (!store.planOpen) return;
    plan = null;
    error = null;
    api.plan(store.desired()).then((p) => (plan = p)).catch((e) => (error = String(e)));
    restore.load().then(() => {
      const risky = Object.keys(store.pending).some((id) => store.tweaks.find((x) => x.id === id)?.risk !== "safe");
      withRestore = !restore.recent && (store.pendingCount >= 3 || risky);
    });
  });

  async function apply() {
    if (withRestore && restore.available && !(await restore.create())) return;
    await store.applyPending();
  }

  const grouped = $derived.by(() => {
    const g = new Map<string, PlannedChange[]>();
    for (const c of plan ?? []) g.set(c.tweak_id, [...(g.get(c.tweak_id) ?? []), c]);
    return [...g.entries()];
  });

  const title = (id: string) => store.tweaks.find((t) => t.id === id)?.title ?? id;

  function verb(id: string): { text: string; cls: string } {
    const p = store.pending[id];
    const tw = store.tweaks.find((x) => x.id === id);
    if (typeof p === "string" && tw) return { text: `→ ${store.choiceLabel(tw, p)}`, cls: "text-accent" };
    return p ? { text: t("plan.apply_one"), cls: "text-safe" } : { text: t("plan.revert_one"), cls: "text-moderate" };
  }
</script>

{#if store.planOpen}
  <div class="fixed inset-0 z-30 bg-black/50" transition:fade={{ duration: 150 }} onclick={() => (store.planOpen = false)} role="presentation"></div>
  <section
    class="fixed top-0 right-0 bottom-0 z-40 flex w-[min(560px,92vw)] flex-col border-l border-line bg-panel shadow-xl shadow-black/50"
    transition:fly={{ x: 560, duration: 240, opacity: 1 }}
    aria-label={t("plan.title")}
  >
    <header class="flex items-center gap-3 border-b border-line px-5 py-4">
      <div class="flex-1">
        <h2 class="font-display text-[16px] font-semibold">{t("plan.title")}</h2>
        <p class="text-[12px] text-mute">{t("plan.subtitle")}</p>
      </div>
      <button class="rounded-lg p-1.5 text-mute hover:bg-card-hi hover:text-ink" onclick={() => (store.planOpen = false)} aria-label={t("common.close")}>
        <X size={18} />
      </button>
    </header>

    <div class="flex-1 space-y-3 overflow-y-auto px-5 py-4">
      {#if error}
        <p class="rounded-lg bg-advanced/10 p-3 text-[13px] text-advanced">{error}</p>
      {:else if !plan}
        <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> {t("plan.computing")}</div>
      {:else}
        {#each grouped as [id, ops] (id)}
          <div class="rounded-md border border-line bg-card p-3">
            <div class="mb-2 flex items-center gap-2 text-[13px]">
              <span class="font-medium">{title(id)}</span>
              <span class="text-[11.5px] {verb(id).cls}">{verb(id).text}</span>
            </div>
            {#each ops as c}
              <div class="mt-1.5 rounded-lg bg-bg/60 p-2 font-mono text-[10.5px] {c.noop ? 'opacity-50' : ''}">
                <div class="break-all text-faint">{c.key}</div>
                <div class="mt-0.5 flex flex-wrap items-center gap-2">
                  <span class="text-ink">{c.name}</span>
                  <span class="text-advanced/90 line-through decoration-advanced/50">{c.from}</span>
                  <ArrowRight size={11} class="text-faint" />
                  <span class="text-safe">{c.to}</span>
                  {#if c.noop}<span class="text-faint">{t("plan.noop")}</span>{/if}
                </div>
              </div>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

    {#if restore.available}
      <div class="border-t border-line px-4 pt-2"><RestoreOption bind:checked={withRestore} /></div>
    {/if}
    <footer class="flex items-center gap-2 border-t border-line px-5 py-4 {restore.available ? 'border-t-0 pt-2' : ''}">
      <span class="flex-1 text-[12px] text-mute">{t("plan.summary", { tweaks: store.pendingCount, values: plan?.filter((c) => !c.noop).length ?? "…" })}</span>
      <button class="rounded-lg px-3 py-2 text-[13px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => (store.planOpen = false)}>{t("common.back")}</button>
      <button
        class="accent-grad inline-flex items-center gap-2 rounded-md px-3.5 py-1.5 text-[13px] font-medium disabled:opacity-50"
        disabled={!plan || store.busy || restore.creating}
        onclick={apply}
      >
        {#if store.busy || restore.creating}<LoaderCircle size={14} class="animate-spin" />{/if}
        {restore.creating ? t("restore.creating") : t("plan.apply")}
      </button>
    </footer>
  </section>
{/if}
