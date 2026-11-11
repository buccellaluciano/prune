<script lang="ts">
  /** Estado de Restaurar sistema: último punto, crear uno y abrir la herramienta de Windows. */
  import { LifeBuoy, LoaderCircle } from "@lucide/svelte";
  import { restoreApi } from "#lib/api.ts";
  import { restore } from "#lib/safety.svelte.ts";
  import { store } from "#lib/store.svelte.ts";
  import { i18n, t } from "#lib/i18n.svelte.ts";
  import { onMount } from "svelte";

  onMount(() => restore.load());

  const fmt = $derived(new Intl.DateTimeFormat(i18n.locale, { dateStyle: "medium", timeStyle: "short" }));
  const s = $derived(restore.status);

  const note = $derived(
    !s ? null : s.policy ? t("restore.policy") : !s.enabled ? t("restore.disabled") : !s.elevated ? t("restore.admin") : null,
  );

  async function open(fn: () => Promise<void>) {
    try {
      await fn();
    } catch (e) {
      store.toast("error", String(e));
    }
  }
</script>

<div class="mb-6 flex items-start gap-3 rounded-lg border border-line bg-card p-4">
  <LifeBuoy size={18} class="mt-0.5 shrink-0 text-accent" />
  <div class="min-w-0 flex-1">
    <div class="text-[13.5px] font-medium">{t("restore.title")}</div>
    <div class="mt-0.5 text-[12px] text-mute">{t("restore.subtitle")}</div>
    {#if !s}
      <div class="mt-2 flex items-center gap-2 text-[12px] text-faint"><LoaderCircle size={13} class="animate-spin" /> {t("common.loading")}</div>
    {:else if note}
      <div class="mt-2 text-[12px] text-moderate">{note}</div>
    {:else if s.last}
      <div class="mt-2 text-[12px] text-faint">{t("restore.last", { when: fmt.format(new Date(s.last.at * 1000)) })}</div>
    {:else if s.readable}
      <div class="mt-2 text-[12px] text-faint">{t("restore.none")}</div>
    {/if}
  </div>
  <div class="flex shrink-0 flex-col items-end gap-1.5">
    {#if restore.available}
      <button
        class="inline-flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-[12.5px] hover:border-mute disabled:opacity-50"
        disabled={restore.creating}
        onclick={() => restore.create()}
      >
        {#if restore.creating}<LoaderCircle size={13} class="animate-spin" /> {t("restore.creating")}{:else}{t("restore.create")}{/if}
      </button>
    {/if}
    {#if s?.elevated && s.enabled}
      <button class="text-[12px] text-mute hover:text-ink" onclick={() => open(restoreApi.open)}>{t("restore.open")}</button>
    {:else if s?.elevated && !s.policy}
      <button class="text-[12px] text-mute hover:text-ink" onclick={() => open(restoreApi.settings)}>{t("restore.settings")}</button>
    {/if}
  </div>
</div>
