<script lang="ts">
  /** "Crear un punto de restauración antes": solo aparece si se puede crear uno. */
  import { Check } from "@lucide/svelte";
  import { restore } from "#lib/safety.svelte.ts";
  import { t } from "#lib/i18n.svelte.ts";

  let { checked = $bindable(false) }: { checked?: boolean } = $props();
</script>

{#if restore.available}
  <button
    role="checkbox"
    aria-checked={checked}
    class="flex items-start gap-2 rounded-md px-1 py-1 text-left text-[12.5px] text-mute hover:text-ink"
    onclick={() => (checked = !checked)}
  >
    <span class="mt-px flex size-3.5 shrink-0 items-center justify-center rounded-[4px] border {checked ? 'border-accent bg-accent text-bg' : 'border-[#3a3935]'}">
      {#if checked}<Check size={10} strokeWidth={3} />{/if}
    </span>
    <span>
      {t("restore.option")}
      <span class="block text-[11px] text-faint">{t(restore.recent ? "restore.option_recent" : "restore.option_hint")}</span>
    </span>
  </button>
{/if}
