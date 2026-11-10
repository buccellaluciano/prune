<script lang="ts">
  /** Más apps: otras apps del mismo autor que P-Rune puede instalar. Por ahora, WMO. */
  import { Activity, ArrowUpCircle, BellOff, Download, ExternalLink, LoaderCircle, Music, Play, ShieldCheck, SlidersVertical, Trash2 } from "@lucide/svelte";
  import { wmoApi, type WmoStatus } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import { onMount } from "svelte";
  import WmoIcon from "./WmoIcon.svelte";

  let status = $state<WmoStatus | null>(null);
  let busy = $state<"install" | "uninstall" | "launch" | null>(null);
  let confirmUninstall = $state(false);

  async function refresh() {
    status = await wmoApi.status().catch(() => null);
  }
  onMount(refresh);

  async function run(kind: "install" | "uninstall" | "launch") {
    busy = kind;
    try {
      if (kind === "install") {
        const wasUpdate = !!status?.installed;
        status = await wmoApi.install();
        store.toast("ok", t(wasUpdate ? "wmo.toast_updated" : "wmo.toast_installed", { version: status.available }));
      } else if (kind === "uninstall") {
        status = await wmoApi.uninstall();
        store.toast("ok", t("wmo.toast_uninstalled"));
      } else {
        await wmoApi.launch();
        setTimeout(refresh, 1500);
      }
    } catch (e) {
      store.toast("error", String(e));
      await refresh();
    } finally {
      busy = null;
      confirmUninstall = false;
    }
  }

  const features = [
    { icon: Music, key: "wmo.f_players" },
    { icon: SlidersVertical, key: "wmo.f_volume" },
    { icon: Activity, key: "wmo.f_stats" },
    { icon: BellOff, key: "wmo.f_notifications" },
  ] as const;
</script>

<div class="mx-auto max-w-3xl">
  <header class="mb-6">
    <h1 class="font-display text-[20px] font-semibold">{t("nav.more")}</h1>
    <p class="mt-1 max-w-2xl text-[13px] text-mute">{t("more.subtitle")}</p>
  </header>

  <section class="overflow-hidden rounded-lg border border-line bg-card">
    <div class="flex flex-wrap items-center gap-4 p-5">
      <WmoIcon size={56} />
      <div class="min-w-0 flex-1">
        <div class="flex flex-wrap items-baseline gap-2">
          <h2 class="text-[16px] font-semibold">WMO</h2>
          <span class="text-[12px] text-faint">Win11 Media Overlay</span>
          {#if status?.installed}
            <span class="rounded bg-safe/15 px-1.5 py-px text-[10.5px] text-safe">
              {t("wmo.installed", { version: status.installed })}{#if status.running} · {t("wmo.running")}{/if}
            </span>
          {/if}
        </div>
        <p class="mt-0.5 text-[13px] text-mute">{t("wmo.tagline")}</p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        {#if !status}
          <LoaderCircle size={16} class="animate-spin text-faint" />
        {:else if !status.installed}
          <button class="accent-grad inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-[12.5px] font-medium disabled:opacity-60" disabled={!!busy} onclick={() => run("install")}>
            {#if busy === "install"}<LoaderCircle size={13} class="animate-spin" /> {t("wmo.installing")}{:else}<Download size={13} /> {t("wmo.install")}{/if}
          </button>
        {:else}
          {#if status.update}
            <button class="accent-grad inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-[12.5px] font-medium disabled:opacity-60" disabled={!!busy} onclick={() => run("install")}>
              {#if busy === "install"}<LoaderCircle size={13} class="animate-spin" /> {t("wmo.installing")}{:else}<ArrowUpCircle size={13} /> {t("wmo.update", { version: status.available })}{/if}
            </button>
          {/if}
          <button class="inline-flex items-center gap-1.5 rounded-md border border-line px-3 py-1.5 text-[12.5px] text-ink hover:border-mute disabled:opacity-60" disabled={!!busy} onclick={() => run("launch")}>
            {#if busy === "launch"}<LoaderCircle size={13} class="animate-spin" />{:else}<Play size={13} />{/if} {t("wmo.open")}
          </button>
          <button
            class="inline-flex items-center gap-1.5 rounded-md px-2 py-1.5 text-[12.5px] text-faint hover:text-advanced disabled:opacity-60"
            disabled={!!busy}
            title={t("wmo.uninstall")}
            aria-label={t("wmo.uninstall")}
            onclick={() => (confirmUninstall = true)}
          >
            <Trash2 size={13} />
          </button>
        {/if}
      </div>
    </div>

    {#if confirmUninstall}
      <div class="flex flex-wrap items-center gap-3 border-t border-line bg-advanced/5 px-5 py-3 text-[12.5px]">
        <span class="flex-1">{t("wmo.uninstall_confirm")}</span>
        <button class="rounded-md px-2.5 py-1 text-mute hover:text-ink" onclick={() => (confirmUninstall = false)}>{t("common.cancel")}</button>
        <button class="inline-flex items-center gap-1.5 rounded-md border border-advanced/50 px-2.5 py-1 text-advanced hover:bg-advanced/10 disabled:opacity-60" disabled={!!busy} onclick={() => run("uninstall")}>
          {#if busy === "uninstall"}<LoaderCircle size={13} class="animate-spin" />{/if} {t("wmo.uninstall")}
        </button>
      </div>
    {/if}

    <div class="border-t border-line p-5">
      <p class="text-[13px] leading-relaxed text-mute">{t("wmo.description")}</p>
      <ul class="mt-4 grid gap-3 sm:grid-cols-2">
        {#each features as f (f.key)}
          <li class="flex gap-2.5 text-[12.5px] leading-snug">
            <f.icon size={15} class="mt-px shrink-0 text-accent" />
            <span>{t(f.key)}</span>
          </li>
        {/each}
      </ul>

      <div class="mt-5 flex gap-2.5 rounded-md border border-line px-3.5 py-3 text-[12.5px]">
        <ShieldCheck size={15} class="mt-px shrink-0 text-safe" />
        <div>
          <div class="font-medium">{t("wmo.privacy_title")}</div>
          <p class="mt-0.5 leading-relaxed text-mute">{t("wmo.privacy")}</p>
        </div>
      </div>
    </div>

    <div class="space-y-1.5 border-t border-line px-5 py-3.5 text-[11.5px] text-faint">
      <p>{t("wmo.where")}</p>
      {#if status}
        <p>
          {t("wmo.verify", { version: status.available })}
          <span class="font-mono break-all text-mute select-text">{status.sha256}</span>
        </p>
      {/if}
      <button class="inline-flex items-center gap-1 text-mute hover:text-ink" onclick={() => wmoApi.openRepo().catch((e) => store.toast("error", String(e)))}>
        <ExternalLink size={11} /> {t("wmo.github")}
      </button>
    </div>
  </section>
</div>
