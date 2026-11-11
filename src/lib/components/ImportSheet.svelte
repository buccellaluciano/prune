<script lang="ts">
  /** Vista previa de una configuración importada: elegir qué traer y pasarlo al flujo de siempre. */
  import { Check, LoaderCircle, X } from "@lucide/svelte";
  import { api } from "#lib/api.ts";
  import { profile, restore, stageAndReview } from "#lib/safety.svelte.ts";
  import { store } from "#lib/store.svelte.ts";
  import { apps } from "#lib/apps.svelte.ts";
  import { i18n, t, tn } from "#lib/i18n.svelte.ts";
  import { fade, fly } from "svelte/transition";
  import RestoreOption from "./RestoreOption.svelte";

  let selected = $state<Record<string, boolean>>({});
  let working = $state(false);
  let withRestore = $state(true);

  const p = $derived(profile.preview);
  const fmt = $derived(new Intl.DateTimeFormat(i18n.locale, { dateStyle: "medium" }));

  $effect(() => {
    if (!p) return;
    const s: Record<string, boolean> = {};
    for (const x of p.tweaks) s[`t:${x.id}`] = !x.current;
    // Las apps "avanzadas" rompen funciones: hay que elegirlas a mano.
    for (const x of p.bloat) s[`b:${x.id}`] = x.tier !== "advanced";
    for (const x of p.apps) s[`a:${x.id}`] = !x.installed;
    selected = s;
    restore.load().then(() => (withRestore = !restore.recent));
  });

  const pendingTweaks = $derived(p?.tweaks.filter((x) => !x.current) ?? []);
  const pendingApps = $derived(p?.apps.filter((x) => !x.installed) ?? []);
  const nothing = $derived(!!p && !pendingTweaks.length && !p.bloat.length && !pendingApps.length);
  const chosen = $derived(Object.keys(selected).filter((key) => selected[key]));

  async function run() {
    if (!p) return;
    working = true;
    try {
      const tw = p.tweaks.filter((x) => selected[`t:${x.id}`]);
      const bl = p.bloat.filter((x) => selected[`b:${x.id}`]).map((x) => x.id);
      const ap = p.apps.filter((x) => selected[`a:${x.id}`]).map((x) => x.id);
      if ((tw.length || bl.length) && withRestore && restore.available && !(await restore.create())) return;
      if (bl.length) {
        const res = await api.removeBloat(bl, false);
        const ok = res.filter((r) => r.ok).length;
        if (ok) store.toast("ok", tn("bloat.removed", ok));
        for (const r of res.filter((r) => !r.ok)) store.toast("error", `${r.id}: ${r.error}`);
      }
      if (ap.length) {
        if (!apps.overview) await apps.load();
        for (const id of ap) apps.enqueue(id, "install");
        store.toast("info", tn("config.apps_queued", ap.length));
      }
      profile.preview = null;
      // El punto de restauración ya se creó: el plan no lo vuelve a ofrecer marcado.
      if (tw.length) stageAndReview(tw.map((x) => (x.choice ? `${x.id}:${x.choice}` : x.id)));
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      working = false;
    }
  }
</script>

{#snippet item(key: string, title: string, detail: string | null, done: boolean)}
  <button
    role="checkbox"
    aria-checked={!!selected[key]}
    disabled={done}
    class="flex w-full items-start gap-2.5 px-3 py-2 text-left hover:bg-card-hi disabled:cursor-default disabled:opacity-50 disabled:hover:bg-transparent"
    onclick={() => (selected = { ...selected, [key]: !selected[key] })}
  >
    <span class="mt-0.5 flex size-3.5 shrink-0 items-center justify-center rounded-[4px] border {selected[key] ? 'border-accent bg-accent text-bg' : 'border-[#3a3935]'}">
      {#if selected[key]}<Check size={10} strokeWidth={3} />{/if}
    </span>
    <span class="min-w-0 flex-1 text-[13px]">
      {title}
      {#if detail}<span class="text-[11.5px] text-accent"> → {detail}</span>{/if}
      {#if done}<span class="text-[11.5px] text-faint"> · {t("config.already")}</span>{/if}
    </span>
  </button>
{/snippet}

{#if p}
  <div class="fixed inset-0 z-30 bg-black/50" transition:fade={{ duration: 150 }} onclick={() => (profile.preview = null)} role="presentation"></div>
  <section
    class="fixed top-0 right-0 bottom-0 z-40 flex w-[min(540px,92vw)] flex-col border-l border-line bg-panel shadow-xl shadow-black/50"
    transition:fly={{ x: 540, duration: 240, opacity: 1 }}
    aria-label={t("config.import_title")}
  >
    <header class="flex items-center gap-3 border-b border-line px-5 py-4">
      <div class="min-w-0 flex-1">
        <h2 class="font-display text-[16px] font-semibold">{t("config.import_title")}</h2>
        <p class="truncate text-[12px] text-mute">
          {t("config.import_subtitle", { file: p.file, version: p.app_version || "?", date: p.created ? fmt.format(new Date(p.created * 1000)) : "?" })}
        </p>
      </div>
      <button class="rounded-lg p-1.5 text-mute hover:bg-card-hi hover:text-ink" onclick={() => (profile.preview = null)} aria-label={t("common.close")}>
        <X size={18} />
      </button>
    </header>

    <div class="flex-1 space-y-5 overflow-y-auto px-5 py-4">
      {#if nothing}
        <p class="rounded-md border border-line bg-card p-4 text-[13px] text-mute">{t("config.nothing")}</p>
      {/if}
      {#if p.tweaks.length}
        <div>
          <h3 class="mb-1.5 text-[12px] font-medium text-mute">{t("config.tweaks")}</h3>
          <div class="divide-y divide-line overflow-hidden rounded-md border border-line bg-card">
            {#each p.tweaks as x (x.id)}{@render item(`t:${x.id}`, x.title, x.choice_label, x.current)}{/each}
          </div>
        </div>
      {/if}
      {#if p.bloat.length}
        <div>
          <h3 class="mb-1.5 text-[12px] font-medium text-mute">{t("config.bloat")}</h3>
          <div class="divide-y divide-line overflow-hidden rounded-md border border-line bg-card">
            {#each p.bloat as x (x.id)}{@render item(`b:${x.id}`, x.title, null, false)}{/each}
          </div>
          <p class="mt-1.5 text-[11.5px] text-faint">{t("config.bloat_now")}</p>
        </div>
      {/if}
      {#if p.apps.length}
        <div>
          <h3 class="mb-1.5 text-[12px] font-medium text-mute">{t("config.apps")}</h3>
          <div class="divide-y divide-line overflow-hidden rounded-md border border-line bg-card">
            {#each p.apps as x (x.id)}{@render item(`a:${x.id}`, x.title, null, x.installed)}{/each}
          </div>
        </div>
      {/if}
      {#if p.skipped}
        <p class="text-[11.5px] text-faint">{tn("config.skipped", p.skipped)}</p>
      {/if}
      {#if !nothing}<p class="text-[11.5px] text-faint">{t("config.apply_hint")}</p>{/if}
    </div>

    {#if restore.available && !nothing}
      <div class="border-t border-line px-4 pt-2"><RestoreOption bind:checked={withRestore} /></div>
    {/if}
    <footer class="flex items-center justify-end gap-2 px-5 py-4 {restore.available && !nothing ? 'pt-2' : 'border-t border-line'}">
      <button class="rounded-lg px-3 py-2 text-[13px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => (profile.preview = null)}>{t("common.back")}</button>
      <button
        class="accent-grad inline-flex items-center gap-2 rounded-md px-3.5 py-1.5 text-[13px] font-medium disabled:opacity-50"
        disabled={!chosen.length || working || nothing}
        onclick={run}
      >
        {#if working}<LoaderCircle size={14} class="animate-spin" />{/if}
        {restore.creating ? t("restore.creating") : t("config.apply")}
      </button>
    </footer>
  </section>
{/if}
