<script lang="ts">
  /** Después de cambiar el DNS: ¿sigue andando internet? Si no se confirma, el backend vuelve atrás solo. */
  import { Globe } from "@lucide/svelte";
  import { network } from "#lib/network.svelte.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import { fade, scale } from "svelte/transition";
</script>

{#if network.pending}
  <div class="fixed inset-0 z-[70] bg-black/55" transition:fade={{ duration: 120 }} role="presentation"></div>
  <div
    class="fixed top-1/2 left-1/2 z-[71] w-[min(420px,92vw)] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-[#3a3935] bg-panel p-5 shadow-xl shadow-black/60"
    transition:scale={{ start: 0.97, duration: 140 }}
    role="alertdialog"
    aria-label={t("dns.confirm_title")}
  >
    <div class="flex items-center gap-2">
      <Globe size={17} class="text-accent" />
      <h2 class="text-[15px] font-semibold">{t("dns.confirm_title")}</h2>
    </div>
    <p class="mt-2 text-[12.5px] leading-relaxed text-mute">{t("dns.confirm_body", { s: network.secondsLeft })}</p>
    <div class="mt-3 h-1 overflow-hidden rounded-full bg-line">
      <div class="h-full rounded-full bg-accent transition-[width] duration-200" style="width:{Math.min(100, (network.secondsLeft / 15) * 100)}%"></div>
    </div>
    <div class="mt-4 flex justify-end gap-2">
      <button class="rounded-md px-3 py-1 text-[12.5px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => network.revert()}>
        {t("dns.back")}
      </button>
      <button class="accent-grad rounded-md px-3.5 py-1 text-[12.5px] font-medium" onclick={() => network.confirm()}>
        {t("dns.keep")}
      </button>
    </div>
  </div>
{/if}
