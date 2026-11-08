<script lang="ts">
  /** Versiones: la instalada, la nueva (si hay) y el historial con un resumen de cada una. */
  import { ArrowUpCircle, LoaderCircle, RefreshCw } from "@lucide/svelte";
  import { appApi } from "#lib/api.ts";
  import { CHANGELOG, compareVersions } from "#lib/changelog.ts";
  import { updates } from "#lib/updates.svelte.ts";
  import { i18n, t } from "#lib/i18n.svelte.ts";
  import { onMount } from "svelte";

  let current = $state<string | null>(null);
  onMount(async () => {
    current = await appApi.version().catch(() => null);
  });

  const released = $derived(current ? CHANGELOG.filter((r) => compareVersions(r.version, current!) <= 0) : []);
  const fmt = $derived(new Intl.DateTimeFormat(i18n.locale, { dateStyle: "long", timeZone: "UTC" }));
</script>

<div class="mx-auto max-w-3xl">
  <header class="mb-6 flex flex-wrap items-end gap-4">
    <div class="min-w-0 flex-1">
      <h1 class="font-display text-[20px] font-semibold">{t("nav.versions")}</h1>
      <p class="mt-1 text-[13px] text-mute">
        {current ? t("versions.current", { version: current }) : t("common.loading")}
      </p>
    </div>
    <button
      class="inline-flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-[12.5px] text-mute hover:text-ink disabled:opacity-50"
      disabled={updates.checking || updates.installing}
      onclick={() => updates.check(false)}
    >
      {#if updates.checking}<LoaderCircle size={13} class="animate-spin" />{:else}<RefreshCw size={13} />{/if}
      {updates.checking ? t("update.checking") : t("update.check")}
    </button>
  </header>

  {#if updates.info}
    {@const u = updates.info}
    <section class="mb-8 rounded-lg border border-accent/40 bg-accent/5 p-4">
      <div class="flex flex-wrap items-center gap-3">
        <ArrowUpCircle size={18} class="shrink-0 text-accent" />
        <div class="min-w-0 flex-1">
          <div class="text-[14px] font-semibold">{t("update.available", { version: u.version })}</div>
          <div class="font-mono text-[11px] text-faint">{u.current} → {u.version}</div>
        </div>
        <button class="accent-grad rounded-md px-3 py-1 text-[12.5px] font-medium" disabled={updates.installing} onclick={() => updates.install()}>
          {t("update.install")}
        </button>
      </div>
      {#if u.notes}
        <p class="mt-3 text-[12.5px] leading-relaxed whitespace-pre-line text-mute">{u.notes}</p>
      {/if}
      <p class="mt-3 text-[11.5px] text-faint">{t("versions.verify_note")}</p>
    </section>
  {/if}

  <ol class="relative space-y-3 border-l border-line pl-5">
    {#each released as r (r.version)}
      {@const installed = current === r.version}
      <li class="relative rounded-md border border-line bg-card px-4 py-3">
        <span class="absolute top-4 -left-[24.5px] size-2 rounded-full {installed ? 'bg-accent' : 'bg-line'}"></span>
        <div class="flex flex-wrap items-baseline gap-2">
          <span class="text-[14px] font-semibold">P-Rune {r.version}</span>
          {#if installed}<span class="rounded bg-accent/15 px-1.5 py-px text-[10.5px] text-accent">{t("versions.installed")}</span>{/if}
          <span class="ml-auto text-[11.5px] text-faint">{fmt.format(new Date(r.date))}</span>
        </div>
        <ul class="mt-2 space-y-1 text-[12.5px] leading-relaxed text-mute">
          {#each r.notes[i18n.lang] as n}
            <li class="flex gap-2"><span class="text-faint" aria-hidden="true">·</span><span>{n}</span></li>
          {/each}
        </ul>
      </li>
    {/each}
  </ol>
</div>
