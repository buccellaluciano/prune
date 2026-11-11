<script lang="ts">
  /** Red: servidores DNS por conexión (IPv4 e IPv6), con DNS cifrado para los proveedores de la lista. */
  import { CircleCheck, CircleX, EthernetPort, Info, LoaderCircle, Lock, RotateCcw, ShieldAlert, Wifi } from "@lucide/svelte";
  import type { AdapterDns, DnsFamily, DohMode, NetAdapter } from "#lib/api.ts";
  import { network } from "#lib/network.svelte.ts";
  import ProxyCard from "./ProxyCard.svelte";
  import { t, type Key } from "#lib/i18n.svelte.ts";
  import { onMount } from "svelte";

  onMount(() => network.load());

  const s = $derived(network.status);
  /** Por qué no se puede cambiar nada ahora (null = se puede). */
  const blocked = $derived(
    !s ? "" : s.domain !== null ? t("dns.domain", { domain: s.domain || "?" }) : s.policy ? t("dns.policy") : !s.elevated ? t("dns.need_admin") : null,
  );

  // ───── Editor (uno a la vez) ─────
  type Mode = "auto" | "provider" | "manual";
  let editing = $state<string | null>(null);
  let mode = $state<Mode>("provider");
  let providerId = $state("cloudflare");
  let useV4 = $state(true);
  let useV6 = $state(true);
  let manual4 = $state(["", ""]);
  let manual6 = $state(["", ""]);
  let doh = $state<DohMode>("preferred");
  let failed = $state<string[] | null>(null);

  function open(a: NetAdapter) {
    editing = a.guid;
    failed = null;
    network.tests = null;
    const cfg = a.v4.config.servers.length ? a.v4 : a.v6;
    if (!a.v4.config.servers.length && !a.v6.config.servers.length) {
      mode = "provider";
      doh = "preferred";
    } else if (cfg.provider) {
      mode = "provider";
      providerId = cfg.provider;
      useV4 = a.v4.config.servers.length > 0;
      useV6 = a.v6.config.servers.length > 0;
      doh = cfg.config.doh === "off" ? "off" : cfg.config.doh;
    } else {
      mode = "manual";
    }
    manual4 = [a.v4.config.servers[0] ?? "", a.v4.config.servers[1] ?? ""];
    manual6 = [a.v6.config.servers[0] ?? "", a.v6.config.servers[1] ?? ""];
  }

  const provider = $derived(s?.providers.find((p) => p.id === providerId));

  function request(a: NetAdapter, m: Mode = mode): AdapterDns {
    const off = { servers: [], doh: "off" as DohMode };
    if (m === "auto") return { guid: a.guid, v4: off, v6: off };
    if (m === "provider" && provider) {
      return {
        guid: a.guid,
        v4: useV4 ? { servers: provider.v4, doh } : off,
        v6: useV6 ? { servers: provider.v6, doh } : off,
      };
    }
    const clean = (l: string[]) => l.map((x) => x.trim()).filter(Boolean);
    return { guid: a.guid, v4: { servers: clean(manual4), doh: "off" }, v6: { servers: clean(manual6), doh: "off" } };
  }

  async function test(a: NetAdapter) {
    failed = null;
    await network.test([request(a)]);
  }

  /** Prueba y aplica; si algún servidor no responde, pide confirmación. */
  async function apply(a: NetAdapter, force = false, m: Mode = mode) {
    const req = request(a, m);
    const hasServers = req.v4.servers.length + req.v6.servers.length > 0;
    if (hasServers && !force) {
      const r = await network.test([req]);
      if (!r) return;
      const bad = r.filter((x) => !x.ok).map((x) => x.server);
      if (bad.length) {
        failed = bad;
        return;
      }
    }
    if (await network.apply([req])) {
      editing = null;
      failed = null;
    }
  }

  function describe(f: DnsFamily): string {
    if (f.config.servers.length) {
      return f.provider ? (s?.providers.find((p) => p.id === f.provider)?.name ?? f.provider) : f.config.servers.join(", ");
    }
    return f.effective.length ? t("dns.auto_from", { servers: f.effective.join(", ") }) : t("dns.auto");
  }

  const custom = (a: NetAdapter) => a.v4.config.servers.length > 0 || a.v6.config.servers.length > 0;

  const DOH: { id: DohMode; label: Key; hint: Key }[] = [
    { id: "preferred", label: "dns.doh_preferred", hint: "dns.doh_preferred_hint" },
    { id: "required", label: "dns.doh_required", hint: "dns.doh_required_hint" },
    { id: "off", label: "dns.doh_off", hint: "dns.doh_off_hint" },
  ];
  const MODES: { id: Mode; label: Key }[] = [
    { id: "provider", label: "dns.mode_provider" },
    { id: "manual", label: "dns.mode_manual" },
    { id: "auto", label: "dns.mode_auto" },
  ];
</script>

<div class="mx-auto max-w-3xl">
  <header class="mb-6">
    <h1 class="font-display text-[20px] font-semibold">{t("nav.network")}</h1>
    <p class="mt-1 max-w-2xl text-[13px] text-mute">{t("dns.subtitle")}</p>
  </header>

  {#if network.loading && !s}
    <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> {t("common.loading")}</div>
  {:else if network.error}
    <p class="rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{network.error}</p>
  {:else if s}
    {#if blocked}
      <div class="mb-4 flex gap-2 rounded-md border border-line px-3.5 py-2.5 text-[12.5px] text-mute">
        <ShieldAlert size={15} class="mt-px shrink-0 text-moderate" /><span>{blocked}</span>
      </div>
    {/if}
    {#if s.vpn}
      <div class="mb-4 flex gap-2 rounded-md border border-line px-3.5 py-2.5 text-[12.5px] text-mute">
        <Info size={15} class="mt-px shrink-0 text-accent" /><span>{t("dns.vpn")}</span>
      </div>
    {/if}

    <h2 class="section-title mb-2 text-[13px] font-medium">{t("dns.section")}</h2>
    <div class="divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
      {#each s.adapters as a (a.guid)}
        <div class="p-4">
          <div class="flex flex-wrap items-center gap-3">
            {#if a.kind === "wifi"}<Wifi size={18} class="shrink-0 text-mute" />{:else}<EthernetPort size={18} class="shrink-0 text-mute" />{/if}
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-baseline gap-2">
                <span class="text-[14px] font-medium">{a.name}</span>
                {#if a.primary}<span class="rounded bg-accent/15 px-1.5 py-px text-[10.5px] text-accent">{t("dns.primary")}</span>{/if}
                <span class="truncate text-[11.5px] text-faint">{a.description}</span>
              </div>
              <dl class="mt-1.5 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-[12.5px]">
                {#each [["IPv4", a.v4], ["IPv6", a.v6]] as const as [label, f] (label)}
                  <dt class="font-mono text-[11px] text-faint">{label}</dt>
                  <dd class="flex min-w-0 flex-wrap items-center gap-1.5 text-mute">
                    <span class="break-all">{describe(f)}</span>
                    {#if f.config.doh !== "off"}
                      <span class="inline-flex items-center gap-1 rounded bg-safe/15 px-1.5 py-px text-[10.5px] text-safe">
                        <Lock size={10} />{f.config.doh === "required" ? t("dns.encrypted_only") : t("dns.encrypted")}
                      </span>
                    {/if}
                  </dd>
                {/each}
              </dl>
            </div>
            {#if editing !== a.guid}
              <div class="flex items-center gap-2">
                {#if custom(a)}
                  <button
                    class="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-[12px] text-faint hover:text-ink disabled:opacity-50"
                    disabled={!!blocked || network.busy || !!network.pending}
                    onclick={() => apply(a, true, "auto")}
                  >
                    <RotateCcw size={12} /> {t("dns.to_auto")}
                  </button>
                {/if}
                <button
                  class="rounded-md border border-line px-3 py-1 text-[12.5px] text-ink hover:border-mute disabled:opacity-50"
                  disabled={!!blocked || !!network.pending}
                  onclick={() => open(a)}
                >
                  {t("dns.change")}
                </button>
              </div>
            {/if}
          </div>

          {#if editing === a.guid}
            <div class="mt-4 rounded-md border border-line bg-panel p-4">
              <div class="flex gap-1 rounded-md border border-line p-0.5" role="radiogroup">
                {#each MODES as m (m.id)}
                  <button
                    role="radio"
                    aria-checked={mode === m.id}
                    class="flex-1 rounded-[4px] px-2 py-1 text-[12.5px] {mode === m.id ? 'bg-card-hi font-medium text-ink' : 'text-mute hover:text-ink'}"
                    onclick={() => ((mode = m.id), (failed = null), (network.tests = null))}
                  >
                    {t(m.label)}
                  </button>
                {/each}
              </div>

              {#if mode === "provider"}
                <div class="mt-3 grid gap-2 sm:grid-cols-2">
                  {#each s.providers as p (p.id)}
                    <button
                      class="rounded-md border px-3 py-2 text-left {providerId === p.id ? 'border-accent bg-accent/5' : 'border-line hover:border-mute'}"
                      onclick={() => ((providerId = p.id), (failed = null), (network.tests = null))}
                    >
                      <div class="text-[13px] font-medium">{p.name}</div>
                      <div class="mt-0.5 text-[11.5px] leading-snug text-mute">{t(`dns.p.${p.id}` as Key)}</div>
                      <div class="mt-1 font-mono text-[10.5px] text-faint">{p.v4.join(" · ")}</div>
                    </button>
                  {/each}
                </div>
                <div class="mt-3 flex flex-wrap gap-4 text-[12.5px]">
                  <label class="inline-flex items-center gap-1.5"><input type="checkbox" bind:checked={useV4} /> {t("dns.use_v4")}</label>
                  <label class="inline-flex items-center gap-1.5"><input type="checkbox" bind:checked={useV6} /> {t("dns.use_v6")}</label>
                </div>

                <div class="mt-4 text-[12px] font-medium">{t("dns.encryption")}</div>
                <div class="mt-1.5 space-y-1.5">
                  {#each DOH as d (d.id)}
                    <label class="flex cursor-pointer gap-2 text-[12.5px]">
                      <input type="radio" name="doh" value={d.id} bind:group={doh} class="mt-0.5" />
                      <span><span class="text-ink">{t(d.label)}</span> <span class="text-faint">· {t(d.hint)}</span></span>
                    </label>
                  {/each}
                </div>
              {:else if mode === "manual"}
                <div class="mt-3 grid gap-3 sm:grid-cols-2">
                  {#each [["IPv4", manual4, "dns.placeholder_v4"], ["IPv6", manual6, "dns.placeholder_v6"]] as const as [label, list, ph] (label)}
                    <div>
                      <div class="mb-1 font-mono text-[11px] text-faint">{label}</div>
                      {#each [0, 1] as i (i)}
                        <input
                          bind:value={list[i]}
                          placeholder={i === 0 ? t(ph) : ""}
                          spellcheck="false"
                          class="mb-1.5 h-8 w-full rounded-md border border-line bg-transparent px-2.5 font-mono text-[12.5px] outline-none placeholder:text-faint focus:border-mute"
                        />
                      {/each}
                    </div>
                  {/each}
                </div>
                <p class="text-[11.5px] text-faint">{t("dns.manual_hint")}</p>
              {:else}
                <p class="mt-3 text-[12.5px] text-mute">{t("dns.mode_auto_hint")}</p>
              {/if}

              {#if network.tests && !failed}
                <ul class="mt-3 space-y-1 text-[12px]">
                  {#each network.tests as r (r.server + r.doh)}
                    <li class="flex items-center gap-1.5 {r.ok ? 'text-safe' : 'text-advanced'}">
                      {#if r.ok}<CircleCheck size={13} />{:else}<CircleX size={13} />{/if}
                      <span class="font-mono">{r.server}</span>
                      <span class="text-mute">{r.ok ? t("dns.test_ok", { ms: r.ms ?? 0 }) : t("dns.test_fail")}{r.doh ? ` · ${t("dns.test_doh")}` : ""}</span>
                    </li>
                  {/each}
                </ul>
              {/if}

              {#if failed}
                <div class="mt-3 rounded-md border border-advanced/40 bg-advanced/5 px-3 py-2.5 text-[12.5px]">
                  <div class="font-medium text-advanced">{t("dns.failed_title")}</div>
                  <div class="mt-1 font-mono text-[12px]">{failed.join(" · ")}</div>
                  <p class="mt-1 text-mute">{t("dns.failed_hint")}</p>
                  <div class="mt-2 flex justify-end gap-2">
                    <button class="rounded-md px-2.5 py-1 text-mute hover:text-ink" onclick={() => (failed = null)}>{t("common.cancel")}</button>
                    <button class="rounded-md border border-advanced/50 px-2.5 py-1 text-advanced hover:bg-advanced/10" onclick={() => apply(a, true)}>
                      {t("dns.apply_anyway")}
                    </button>
                  </div>
                </div>
              {/if}

              {#if !failed}
                <div class="mt-4 flex flex-wrap items-center justify-end gap-2">
                  <button class="rounded-md px-2.5 py-1 text-[12.5px] text-mute hover:text-ink" onclick={() => (editing = null)}>{t("common.cancel")}</button>
                  {#if mode !== "auto"}
                    <button
                      class="rounded-md border border-line px-3 py-1 text-[12.5px] hover:border-mute disabled:opacity-50"
                      disabled={network.busy}
                      onclick={() => test(a)}
                    >
                      {t("dns.test")}
                    </button>
                  {/if}
                  <button
                    class="accent-grad inline-flex items-center gap-1.5 rounded-md px-3.5 py-1 text-[12.5px] font-medium disabled:opacity-50"
                    disabled={network.busy || !!network.pending}
                    onclick={() => apply(a)}
                  >
                    {#if network.busy}<LoaderCircle size={13} class="animate-spin" />{/if}
                    {t("dns.apply")}
                  </button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {:else}
        <p class="p-4 text-[13px] text-mute">{t("dns.no_adapters")}</p>
      {/each}
    </div>

    <ProxyCard />

    <section class="mt-6 rounded-md border border-line px-4 py-3 text-[12.5px]">
      <div class="font-medium">{t("dns.notes_title")}</div>
      <ul class="mt-1.5 space-y-1 leading-relaxed text-mute">
        <li>· {t("dns.note_adapter")}</li>
        <li>· {t("dns.note_portal")}</li>
        <li>· {t("dns.note_parental")}</li>
      </ul>
    </section>
  {/if}
</div>
