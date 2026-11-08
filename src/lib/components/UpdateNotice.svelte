<script lang="ts">
  /** Aviso discreto en la barra lateral cuando hay una versión nueva. */
  import { ArrowUpCircle, X } from "@lucide/svelte";
  import { updates } from "#lib/updates.svelte.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import { slide } from "svelte/transition";

  let showNotes = $state(false);
</script>

{#if updates.info && !updates.dismissed}
  {@const u = updates.info}
  <div transition:slide={{ duration: 160 }} class="update-notice mx-2.5 mb-2 rounded-md border border-accent/40 bg-accent/5 px-3 py-2.5">
    <div class="flex items-start gap-2">
      <ArrowUpCircle size={15} class="mt-px shrink-0 text-accent" />
      <div class="min-w-0 flex-1">
        <div class="text-[12.5px] font-medium text-ink">{t("update.available", { version: u.version })}</div>
        <div class="font-mono text-[10.5px] text-faint">{u.current} → {u.version}</div>
      </div>
      <button class="rounded p-0.5 text-faint hover:text-ink" onclick={() => (updates.dismissed = true)} aria-label={t("update.later")} title={t("update.later")}>
        <X size={13} />
      </button>
    </div>
    {#if showNotes && u.notes}
      <p transition:slide={{ duration: 140 }} class="mt-2 max-h-40 overflow-y-auto text-[11.5px] leading-relaxed whitespace-pre-line text-mute">{u.notes}</p>
    {/if}
    <div class="mt-2 flex items-center gap-2">
      <button class="accent-grad rounded-md px-2.5 py-1 text-[12px] font-medium" onclick={() => updates.install()}>
        {t("update.install")}
      </button>
      {#if u.notes}
        <button class="text-[11.5px] text-mute hover:text-ink" onclick={() => (showNotes = !showNotes)}>
          {showNotes ? t("update.hide_notes") : t("update.notes")}
        </button>
      {/if}
    </div>
  </div>
{/if}
