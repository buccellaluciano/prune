<script lang="ts">
  import { Check, ChevronRight, CircleAlert, Copy, ExternalLink, LoaderCircle, ShieldCheck, X } from "@lucide/svelte";
  import { vtApi } from "#lib/api.ts";
  import { apps, STEPS, type Step } from "#lib/apps.svelte.ts";
  import { store } from "#lib/store.svelte.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import AppIcon from "./AppIcon.svelte";
  import { fade, scale } from "svelte/transition";

  let showLog = $state(false);
  let copied = $state(false);

  const r = $derived(apps.running);
  const steps = $derived(r ? STEPS[r.action] : []);
  const current = $derived(r ? steps.indexOf(r.step) : -1);
  const sha = $derived(r?.check?.installer?.sha256 ?? null);
  const reviewing = $derived(r?.review === "review");

  const LABEL = $derived<Record<Step, string>>({
    verify: t("install.step.verify"),
    download: t("install.step.download"),
    hash: t("install.step.hash"),
    install: t("install.step.install"),
    uninstall: t("install.step.uninstall"),
    done: t("install.step.done"),
  });

  function stepState(i: number): "done" | "active" | "pending" | "failed" {
    if (!r) return "pending";
    if (r.result) {
      if (r.result.ok) return "done";
      return i < current ? "done" : i === current ? "failed" : "pending";
    }
    return i < current ? "done" : i === current ? "active" : "pending";
  }

  function hint(step: Step): string | null {
    if (!r) return null;
    if (step === "verify") return r.check ? null : t("install.reading_manifest");
    if (step === "download" && r.step === "download") return r.detail || t("install.connecting");
    if (step === "hash") return t("install.hash_hint");
    return null;
  }

  /** Copia con la API del portapapeles y, si el navegador no la permite, con el método clásico. */
  function copyFallback(text: string): boolean {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.cssText = "position:fixed;opacity:0;pointer-events:none";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    return ok;
  }

  async function copy() {
    if (!sha) return;
    let ok = false;
    try {
      await navigator.clipboard.writeText(sha);
      ok = true;
    } catch {
      ok = copyFallback(sha);
    }
    if (ok) {
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } else store.toast("error", t("install.copy_failed"));
  }

  function openVt() {
    if (sha) vtApi.open(sha).catch((e) => store.toast("error", String(e)));
  }
</script>

{#if apps.sheetOpen && r}
  <div class="fixed inset-0 z-40 bg-black/50" transition:fade={{ duration: 120 }} role="presentation" onclick={() => !r.review && apps.closeSheet()}></div>
  <div
    class="fixed top-1/2 left-1/2 z-50 w-[min(540px,92vw)] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-lg border border-[#3a3935] bg-panel shadow-xl shadow-black/60"
    transition:scale={{ start: 0.97, duration: 140 }}
    role="dialog"
    aria-label={t("install.title")}
  >
    <header class="flex items-center gap-3 border-b border-line px-5 py-4">
      <AppIcon id={r.id} title={apps.title(r.id)} size={40} />
      <div class="min-w-0 flex-1">
        <div class="truncate text-[15px] font-semibold">{apps.title(r.id)}</div>
        <div class="truncate font-mono text-[11px] text-faint">
          {r.id}{#if r.check?.installer?.version} · {r.check.installer.version}{/if}
        </div>
      </div>
      {#if !r.review}
        <button class="rounded p-1.5 text-faint hover:bg-card-hi hover:text-ink" onclick={() => apps.closeSheet()} aria-label={r.result ? t("common.close") : t("install.hide")} title={r.result ? t("common.close") : t("install.hide_title")}>
          <X size={16} />
        </button>
      {/if}
    </header>

    <ol class="px-5 py-4">
      {#each steps as step, i (step)}
        {@const s = stepState(i)}
        {@const h = hint(step)}
        <li class="relative flex gap-3 pb-4 last:pb-0">
          {#if i < steps.length - 1}
            <span class="absolute top-6 bottom-0 left-[9.5px] w-px {s === 'done' ? 'bg-accent/60' : 'bg-line'}" aria-hidden="true"></span>
          {/if}
          <span
            class="relative z-10 grid size-5 shrink-0 place-items-center rounded-full border
              {s === 'done' ? 'border-accent bg-accent text-bg' : s === 'active' ? 'border-accent text-accent' : s === 'failed' ? 'border-advanced text-advanced' : 'border-line text-faint'}"
          >
            {#if s === "done"}<Check size={11} strokeWidth={3} />
            {:else if s === "active"}<LoaderCircle size={11} class="animate-spin" />
            {:else if s === "failed"}<X size={11} strokeWidth={3} />{/if}
          </span>
          <div class="min-w-0 flex-1">
            <div class="text-[13px] {s === 'pending' ? 'text-faint' : 'text-ink'}">{LABEL[step]}</div>
            {#if h && s !== "pending"}
              <div class="mt-0.5 truncate text-[12px] text-mute" title={h}>{h}</div>
            {/if}

            <!-- Hash del instalador: lo mismo que winget va a verificar al descargar. -->
            {#if step === "verify" && sha}
              <div class="mt-1.5 rounded-md border border-line bg-bg/60 p-2.5">
                <div class="text-[11px] text-faint">{t("install.sha")}</div>
                <div class="mt-0.5 font-mono text-[11.5px] break-all text-ink select-text">{sha}</div>
                <div class="mt-2 flex flex-wrap gap-1.5">
                  <button class="inline-flex items-center gap-1.5 rounded-md border border-line px-2 py-1 text-[11.5px] text-mute hover:text-ink" onclick={copy}>
                    {#if copied}<Check size={12} class="text-safe" /> {t("install.copied")}{:else}<Copy size={12} /> {t("install.copy")}{/if}
                  </button>
                  <button class="inline-flex items-center gap-1.5 rounded-md border border-line px-2 py-1 text-[11.5px] text-mute hover:text-ink" onclick={openVt}>
                    <ExternalLink size={12} /> {t("install.open_vt")}
                  </button>
                </div>
                {#if r.check?.installer?.url}
                  <div class="mt-2 truncate font-mono text-[10.5px] text-faint" title={r.check.installer.url}>{r.check.installer.url}</div>
                {/if}
              </div>
            {/if}

            {#if step === "download" && r.step === "download" && !r.result}
              <div class="mt-1.5 h-1 overflow-hidden rounded-full bg-line">
                {#if r.fraction !== null}
                  <div class="h-full rounded-full bg-accent transition-[width] duration-200" style="width:{Math.round(r.fraction * 100)}%"></div>
                {:else}
                  <div class="h-full w-1/3 animate-pulse rounded-full bg-accent/60"></div>
                {/if}
              </div>
            {/if}
          </div>
        </li>
      {/each}
    </ol>

    {#if r.review}
      <div class="mx-5 mb-4 rounded-md border p-3 text-[12.5px] {reviewing ? 'border-line' : 'border-moderate/40 bg-moderate/5'}">
        {#if reviewing}
          <div class="flex gap-2 text-mute">
            <ShieldCheck size={15} class="mt-px shrink-0 text-accent" />
            <span>{t("install.review_hint")}</span>
          </div>
        {:else}
          <div class="flex gap-2 text-moderate"><CircleAlert size={15} class="mt-px shrink-0" /> {r.review}</div>
        {/if}
        <div class="mt-3 flex items-center justify-end gap-2">
          {#if reviewing}
            <label class="mr-auto flex cursor-pointer items-center gap-1.5 text-[11.5px] text-faint hover:text-mute">
              <input type="checkbox" class="accent-[#8fb5a0]" checked={!apps.reviewFirst} onchange={(e) => apps.setReviewFirst(!e.currentTarget.checked)} />
              {t("install.dont_stop")}
            </label>
          {/if}
          <button class="rounded-md px-3 py-1 text-[12.5px] text-mute hover:bg-card-hi hover:text-ink" onclick={() => apps.decide(false)}>{t("common.cancel")}</button>
          <button
            class="rounded-md px-3 py-1 text-[12.5px] font-medium {reviewing ? 'accent-grad' : 'border border-moderate/60 text-moderate hover:bg-moderate/10'}"
            onclick={() => apps.decide(true)}
          >
            {reviewing ? (r.action === "upgrade" ? t("apps.upgrade") : t("apps.install")) : t("install.install_anyway")}
          </button>
        </div>
      </div>
    {/if}

    {#if r.result}
      <div class="mx-5 mb-4 flex items-center gap-2 rounded-md border p-3 text-[12.5px] {r.result.ok ? 'border-safe/40 text-safe' : 'border-advanced/40 text-advanced'}">
        {#if r.result.ok}<Check size={15} />{:else}<CircleAlert size={15} />{/if}
        <span class="min-w-0 flex-1">{r.result.message}</span>
      </div>
    {/if}

    <footer class="border-t border-line px-5 py-2.5">
      <button class="inline-flex items-center gap-1 text-[12px] text-faint hover:text-ink" onclick={() => (showLog = !showLog)} aria-expanded={showLog}>
        <ChevronRight size={12} class="transition-transform {showLog ? 'rotate-90' : ''}" /> {t("install.details")}
      </button>
      {#if showLog}
        <pre class="mt-2 max-h-40 overflow-y-auto rounded-md border border-line bg-bg/60 p-2 font-mono text-[10.5px] leading-relaxed whitespace-pre-wrap text-mute">{r.log.length ? r.log.join("\n") : t("install.no_output")}</pre>
      {/if}
      {#if apps.queue.length}
        <div class="mt-1.5 text-[11.5px] text-faint">{t("install.next", { list: apps.queue.map((j) => apps.title(j.id)).join(", ") })}</div>
      {/if}
    </footer>
  </div>
{/if}
