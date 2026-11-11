<script lang="ts">
  /** Proxy del usuario: activar o desactivar lo que ya está configurado (no se crean ni editan proxies). */
  import { Info, LoaderCircle, ShieldAlert } from "@lucide/svelte";
  import { proxyApi, type ProxyKind, type ProxyStatus } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import { t, type Key } from "#lib/i18n.svelte.ts";
  import { onMount } from "svelte";

  let s = $state<ProxyStatus | null>(null);
  let error = $state<string | null>(null);
  let busy = $state<ProxyKind | null>(null);

  async function load() {
    try {
      s = await proxyApi.status();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);

  const locked = $derived(
    !s ? null : s.foreign_user ? t("common.foreign_admin") : s.policy ? t("proxy.policy") : null,
  );

  async function toggle(kind: ProxyKind, enabled: boolean) {
    busy = kind;
    try {
      const id = await proxyApi.set(kind, enabled);
      if (id !== null) store.toast("ok", t(enabled ? "proxy.turned_on" : "proxy.turned_off", { what: t(LABEL[kind]) }));
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      busy = null;
      await load();
    }
  }

  const LABEL: Record<ProxyKind, Key> = { manual: "proxy.manual", pac: "proxy.pac", auto_detect: "proxy.auto_detect" };

  const rows = $derived(
    !s
      ? []
      : [
          {
            kind: "manual" as const,
            on: s.manual,
            detail: s.server ? (s.bypass ? `${s.server} · ${t("proxy.except", { list: s.bypass })}` : s.server) : null,
            hint: "proxy.manual_hint" as Key,
          },
          { kind: "pac" as const, on: s.pac, detail: s.pac_url || null, hint: "proxy.pac_hint" as Key },
          { kind: "auto_detect" as const, on: s.auto_detect, detail: null, hint: "proxy.auto_detect_hint" as Key },
        ],
  );
</script>

<h2 class="section-title mt-8 mb-2 text-[13px] font-medium">{t("proxy.section")}</h2>
<p class="-mt-1 mb-3 text-[12.5px] text-mute">{t("proxy.subtitle")}</p>

{#if error}
  <p class="rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{error}</p>
{:else if !s}
  <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> {t("common.loading")}</div>
{:else}
  {#if locked}
    <div class="mb-3 flex gap-2 rounded-md border border-line px-3.5 py-2.5 text-[12.5px] text-mute">
      <ShieldAlert size={15} class="mt-px shrink-0 text-moderate" /><span>{locked}</span>
    </div>
  {/if}
  <div class="divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
    {#each rows as r (r.kind)}
      {@const configured = r.kind === "auto_detect" || !!r.detail}
      {@const disabled = !!locked || busy !== null || (!r.on && !configured)}
      <div class="flex items-start gap-4 p-4">
        <div class="min-w-0 flex-1">
          <div class="text-[13.5px] font-medium">{t(LABEL[r.kind])}</div>
          <div class="mt-0.5 text-[12px] text-mute">{t(r.hint)}</div>
          {#if r.detail}
            <div class="mt-1 font-mono text-[11.5px] break-all text-faint">{r.detail}</div>
          {:else if r.kind !== "auto_detect"}
            <div class="mt-1 text-[11.5px] text-faint">{t("proxy.not_configured")}</div>
          {/if}
          {#if r.kind === "manual" && s.local && s.server}
            <div class="mt-1.5 flex gap-1.5 text-[11.5px] text-moderate">
              <Info size={12} class="mt-px shrink-0" /><span>{t("proxy.local")}</span>
            </div>
          {/if}
        </div>
        <button
          role="switch"
          aria-checked={r.on}
          aria-label={t(LABEL[r.kind])}
          title={!configured && !r.on ? t("proxy.not_configured") : undefined}
          {disabled}
          onclick={() => toggle(r.kind, !r.on)}
          class="relative mt-0.5 h-5 w-9 shrink-0 rounded-full border transition-colors duration-150
            {r.on ? 'border-accent bg-accent' : 'border-[#3a3935] bg-transparent'} {disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}"
        >
          <span class="absolute top-[3px] left-[3px] size-3 rounded-full transition-transform duration-150 {r.on ? 'translate-x-4 bg-bg' : 'bg-mute'}"></span>
        </button>
      </div>
    {/each}
  </div>
  {#if s.winhttp}
    <p class="mt-2 text-[11.5px] text-faint">{t("proxy.winhttp", { proxy: s.winhttp })}</p>
  {/if}
{/if}
