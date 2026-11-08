<script lang="ts">
  /** Progreso de la actualización: descarga, verificación de la firma y cierre de la app. */
  import { CircleAlert, LoaderCircle, ShieldCheck } from "@lucide/svelte";
  import { updates } from "#lib/updates.svelte.ts";
  import { bytes } from "#lib/format.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import { fade, scale } from "svelte/transition";

  const pct = $derived(updates.total ? Math.min(100, Math.round((updates.downloaded / updates.total) * 100)) : null);
</script>

{#if updates.installing && updates.info}
  <div class="fixed inset-0 z-[70] bg-black/55" transition:fade={{ duration: 120 }} role="presentation"></div>
  <div
    class="fixed top-1/2 left-1/2 z-[71] w-[min(440px,92vw)] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-[#3a3935] bg-panel p-5 shadow-xl shadow-black/60"
    transition:scale={{ start: 0.97, duration: 140 }}
    role="dialog"
    aria-label={t("update.title", { version: updates.info.version })}
  >
    <h2 class="text-[15px] font-semibold">{t("update.title", { version: updates.info.version })}</h2>

    {#if updates.error}
      <div class="mt-3 flex gap-2 text-[12.5px] text-advanced">
        <CircleAlert size={15} class="mt-px shrink-0" />
        <span class="min-w-0 break-words">{updates.error}</span>
      </div>
      <div class="mt-4 flex justify-end">
        <button class="rounded-md px-3 py-1 text-[12.5px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => updates.close()}>
          {t("common.close")}
        </button>
      </div>
    {:else if updates.verifying}
      <div class="mt-3 flex gap-2 text-[12.5px] text-mute">
        <ShieldCheck size={15} class="mt-px shrink-0 text-accent" />
        <span>{t("update.verifying")}</span>
      </div>
    {:else if updates.launching}
      <div class="mt-3 flex gap-2 text-[12.5px] text-safe">
        <ShieldCheck size={15} class="mt-px shrink-0" />
        <span>{t("update.launching")}</span>
      </div>
    {:else}
      <div class="mt-3 flex items-center gap-2 text-[12.5px] text-mute">
        <LoaderCircle size={13} class="animate-spin" />
        {t("update.downloading")}
        {#if updates.total}<span class="ml-auto font-mono text-[11px]">{bytes(updates.downloaded)} / {bytes(updates.total)}</span>{/if}
      </div>
      <div class="mt-2 h-1 overflow-hidden rounded-full bg-line">
        {#if pct !== null}
          <div class="h-full rounded-full bg-accent transition-[width] duration-200" style="width:{pct}%"></div>
        {:else}
          <div class="h-full w-1/3 animate-pulse rounded-full bg-accent/60"></div>
        {/if}
      </div>
      <p class="mt-3 text-[11.5px] leading-relaxed text-faint">{t("update.hint")}</p>
    {/if}
  </div>
{/if}
