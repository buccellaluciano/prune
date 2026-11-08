<script lang="ts">
  /**
   * Limpieza de archivos descartables (dentro de Disco). Primero mide; nada se borra sin
   * elegirlo y confirmarlo. Las categorías y las carpetas las define el backend.
   */
  import { Check, LoaderCircle, Sparkles, TriangleAlert } from "@lucide/svelte";
  import { cleanupApi, type CleanupId, type CleanView } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import { bytes } from "#lib/format.ts";
  import { t, tn, type Key } from "#lib/i18n.svelte.ts";

  let views = $state<CleanView[] | null>(null);
  let measuring = $state(false);
  let working = $state(false);
  let confirming = $state(false);
  let selected = $state<Partial<Record<CleanupId, boolean>>>({});

  const elevated = $derived(store.overview?.system.elevated ?? false);
  const chosen = $derived((views ?? []).filter((v) => selected[v.id] && v.available && v.bytes > 0));
  const total = $derived(chosen.reduce((s, v) => s + v.bytes, 0));

  async function measure() {
    measuring = true;
    confirming = false;
    try {
      const v = await cleanupApi.scan();
      // Por defecto, lo que no tiene advertencias y ocupa algo.
      const keep = views ? selected : null;
      selected = Object.fromEntries(v.map((x) => [x.id, keep ? !!keep[x.id] : x.available && x.bytes > 0 && !x.warn]));
      views = v;
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      measuring = false;
    }
  }

  async function clean() {
    working = true;
    try {
      const res = await cleanupApi.run(chosen.map((v) => v.id));
      const freed = res.reduce((s, r) => s + r.freed, 0);
      const skipped = res.reduce((s, r) => s + r.skipped, 0);
      store.toast("ok", freed > 0 ? t("cleanup.done", { size: bytes(freed) }) : t("cleanup.nothing"));
      if (skipped > 0) store.toast("info", tn("cleanup.skipped", skipped));
      for (const r of res.filter((x) => x.error)) store.toast("error", `${t(`cleanup.cat.${r.id}` as Key)}: ${r.error}`);
      selected = {};
      views = null;
      await measure();
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      working = false;
      confirming = false;
    }
  }

  function reason(v: CleanView): string | null {
    if (!v.available) return v.admin && !elevated ? t("cleanup.needs_admin") : t("cleanup.foreign");
    if (v.bytes === 0) return t("cleanup.empty");
    return null;
  }
</script>

<section class="mb-8 rounded-lg border border-line bg-card">
  <header class="flex flex-wrap items-center gap-3 px-4 py-3">
    <Sparkles size={16} strokeWidth={1.7} class="shrink-0 text-faint" />
    <div class="min-w-0 flex-1">
      <h2 class="text-[13.5px] font-medium">{t("cleanup.title")}</h2>
      <p class="text-[12px] leading-relaxed text-mute">{t("cleanup.subtitle")}</p>
    </div>
    <button
      class="inline-flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:border-[#3a3935] hover:bg-line disabled:opacity-50"
      disabled={measuring || working}
      onclick={measure}
    >
      {#if measuring}<LoaderCircle size={13} class="animate-spin" />{/if}
      {views ? t("cleanup.remeasure") : t("cleanup.measure")}
    </button>
  </header>

  {#if views}
    <div class="divide-y divide-line border-t border-line">
      {#each views as v (v.id)}
        {@const why = reason(v)}
        {@const on = !why && !!selected[v.id]}
        <button
          role="checkbox"
          aria-checked={on}
          disabled={!!why || working}
          class="group flex w-full items-start gap-3 px-4 py-2.5 text-left transition-colors enabled:hover:bg-card-hi disabled:cursor-default"
          onclick={() => ((selected = { ...selected, [v.id]: !selected[v.id] }), (confirming = false))}
        >
          <span
            class="mt-0.5 grid size-4 shrink-0 place-items-center rounded-[4px] border transition-colors
              {on ? 'border-accent bg-accent text-bg' : why ? 'border-line' : 'border-[#3a3935] group-hover:border-mute'}"
            aria-hidden="true"
          >
            {#if on}<Check size={11} strokeWidth={3} />{/if}
          </span>
          <span class="min-w-0 flex-1 {why ? 'opacity-60' : ''}">
            <span class="block text-[13px] font-medium">{t(`cleanup.cat.${v.id}` as Key)}</span>
            <span class="block text-[12px] leading-relaxed text-mute">{t(`cleanup.hint.${v.id}` as Key)}</span>
            {#if v.warn}
              <span class="mt-0.5 flex gap-1.5 text-[12px] leading-relaxed text-moderate">
                <TriangleAlert size={12} class="mt-[3px] shrink-0" />{t(`cleanup.warn.${v.warn}` as Key)}
              </span>
            {/if}
          </span>
          <span class="shrink-0 pt-0.5 text-right">
            {#if why}
              <span class="text-[11.5px] text-faint">{why}</span>
            {:else}
              <span class="block font-mono text-[12px]">{bytes(v.bytes)}</span>
              <span class="block font-mono text-[10.5px] text-faint">{tn("disk.files", v.files, { n: v.files.toLocaleString() })}</span>
            {/if}
          </span>
        </button>
      {/each}
    </div>

    <footer class="flex flex-wrap items-center justify-end gap-2 border-t border-line px-4 py-2.5">
      {#if confirming}
        <span class="mr-auto text-[12.5px] text-moderate">{t("cleanup.confirm", { size: bytes(total) })}</span>
        <button class="rounded-md px-2.5 py-1 text-[12.5px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => (confirming = false)}>
          {t("common.cancel")}
        </button>
        <button
          class="inline-flex items-center gap-2 rounded-md border border-advanced/60 px-3 py-1 text-[12.5px] font-medium text-advanced hover:bg-advanced/10 disabled:opacity-50"
          disabled={working}
          onclick={clean}
        >
          {#if working}<LoaderCircle size={13} class="animate-spin" />{/if}
          {t("cleanup.yes")}
        </button>
      {:else}
        <button
          class="accent-grad rounded-md px-3 py-1 text-[12.5px] font-medium disabled:opacity-40"
          disabled={!chosen.length || measuring}
          onclick={() => (confirming = true)}
        >
          {t("cleanup.clean", { size: bytes(total) })}
        </button>
      {/if}
    </footer>
  {/if}
</section>
