<script lang="ts">
  /** Windows Update: pausar y horas activas (los mismos valores que escribe Configuración). */
  import { LoaderCircle, ShieldAlert } from "@lucide/svelte";
  import { wuApi, type WuRequest, type WuStatus } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import { i18n, t, tn } from "#lib/i18n.svelte.ts";
  import { onMount } from "svelte";

  let s = $state<WuStatus | null>(null);
  let error = $state<string | null>(null);
  let busy = $state<"pause" | "hours" | null>(null);
  let weeks = $state(1);
  let mode = $state<"auto" | "manual">("manual");
  let start = $state(8);
  let end = $state(17);

  const MAX_SPAN = 18;

  async function load() {
    try {
      s = await wuApi.status();
      error = null;
      if (s.active_start !== null) start = s.active_start;
      if (s.active_end !== null) end = s.active_end;
      mode = s.smart_hours ? "auto" : "manual";
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);

  const dateFmt = $derived(new Intl.DateTimeFormat(i18n.locale, { dateStyle: "long" }));
  const maxWeeks = $derived(Math.max(1, Math.floor((s?.max_pause_days ?? 35) / 7)));
  const span = $derived((end - start + 24) % 24);
  const spanOk = $derived(span > 0 && span <= MAX_SPAN);
  const hourLabel = (h: number) => `${String(h).padStart(2, "0")}:00`;
  const hoursChanged = $derived(
    !!s && (mode === "auto" ? s.smart_hours !== true : s.smart_hours === true || s.active_start !== start || s.active_end !== end),
  );

  async function send(kind: "pause" | "hours", request: WuRequest, ok: string) {
    busy = kind;
    try {
      // null: ya estaba así, no se cambió nada.
      if ((await wuApi.set(request)) !== null) store.toast("ok", ok);
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      busy = null;
      await load();
    }
  }

  function pause() {
    const days = Math.min(weeks * 7, s?.max_pause_days ?? 35);
    const until = dateFmt.format(new Date(Date.now() + days * 86400_000));
    send("pause", { action: "pause", days }, t("updates.paused", { date: until }));
  }

  function saveHours() {
    send(
      "hours",
      mode === "auto" ? { action: "hours", start: null, end: null } : { action: "hours", start, end },
      t("updates.hours_saved"),
    );
  }
</script>

{#if error}
  <p class="mb-6 rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{error}</p>
{:else if !s}
  <div class="mb-6 flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> {t("common.loading")}</div>
{:else}
  {#if !s.elevated}
    <div class="mb-3 flex gap-2 rounded-md border border-line px-3.5 py-2.5 text-[12.5px] text-mute">
      <ShieldAlert size={15} class="mt-px shrink-0 text-moderate" /><span>{t("updates.admin")}</span>
    </div>
  {/if}
  <div class="mb-8 divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
    <!-- Pausa -->
    <div class="p-4">
      <div class="flex items-start gap-4">
        <div class="min-w-0 flex-1">
          <div class="text-[13.5px] font-medium">{t("updates.pause_title")}</div>
          <div class="mt-0.5 text-[12px] text-mute">{t("updates.pause_hint")}</div>
          <div class="mt-1.5 text-[12px] {s.paused_until ? 'text-moderate' : 'text-faint'}">
            {s.paused_until ? t("updates.paused_until", { date: dateFmt.format(new Date(s.paused_until * 1000)) }) : t("updates.not_paused")}
          </div>
          {#if s.pause_policy}<div class="mt-1 text-[11.5px] text-moderate">{t("updates.policy")}</div>{/if}
        </div>
      </div>
      {#if !s.pause_policy}
        <div class="mt-3 flex flex-wrap items-center gap-2">
          <select
            bind:value={weeks}
            disabled={!s.elevated || busy !== null}
            class="h-7 rounded-md border border-line bg-panel px-2 text-[12.5px] outline-none disabled:opacity-50"
            aria-label={t("updates.pause_title")}
          >
            {#each Array.from({ length: maxWeeks }, (_, i) => i + 1) as w}
              <option value={w}>{tn("updates.weeks", w)}</option>
            {/each}
          </select>
          <button
            class="inline-flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-[12.5px] hover:border-mute disabled:opacity-50"
            disabled={!s.elevated || busy !== null}
            onclick={pause}
          >
            {#if busy === "pause"}<LoaderCircle size={13} class="animate-spin" />{/if}
            {s.paused_until ? t("updates.extend") : t("updates.pause")}
          </button>
          {#if s.paused_until}
            <button
              class="rounded-md px-2.5 py-1 text-[12.5px] text-mute hover:bg-card-hi hover:text-ink disabled:opacity-50"
              disabled={!s.elevated || busy !== null}
              onclick={() => send("pause", { action: "resume" }, t("updates.resumed"))}
            >
              {t("updates.resume")}
            </button>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Horas activas -->
    <div class="p-4">
      <div class="text-[13.5px] font-medium">{t("updates.hours_title")}</div>
      <div class="mt-0.5 text-[12px] text-mute">{t("updates.hours_hint", { max: MAX_SPAN })}</div>
      {#if s.active_start !== null && s.active_end !== null && s.smart_hours !== true}
        <div class="mt-1.5 text-[12px] text-faint">{t("updates.hours_current", { start: hourLabel(s.active_start), end: hourLabel(s.active_end) })}</div>
      {/if}
      {#if s.hours_policy}
        <div class="mt-1 text-[11.5px] text-moderate">{t("updates.policy")}</div>
      {:else}
        <div class="mt-3 flex flex-wrap items-center gap-2 text-[12.5px]">
          <div class="flex overflow-hidden rounded-md border border-line">
            {#each [["auto", "updates.hours_auto"], ["manual", "updates.hours_manual"]] as [m, label]}
              <button
                class="px-2.5 py-1 {mode === m ? 'bg-card-hi text-ink' : 'text-mute hover:text-ink'} disabled:opacity-50"
                disabled={!s.elevated || busy !== null}
                onclick={() => (mode = m as "auto" | "manual")}
              >
                {t(label as "updates.hours_auto")}
              </button>
            {/each}
          </div>
          {#if mode === "manual"}
            <span class="text-mute">{t("updates.hours_from")}</span>
            <select bind:value={start} disabled={!s.elevated || busy !== null} class="h-7 rounded-md border border-line bg-panel px-2 outline-none disabled:opacity-50" aria-label={t("updates.hours_from")}>
              {#each Array.from({ length: 24 }, (_, h) => h) as h}<option value={h}>{hourLabel(h)}</option>{/each}
            </select>
            <span class="text-mute">{t("updates.hours_to")}</span>
            <select bind:value={end} disabled={!s.elevated || busy !== null} class="h-7 rounded-md border border-line bg-panel px-2 outline-none disabled:opacity-50" aria-label={t("updates.hours_to")}>
              {#each Array.from({ length: 24 }, (_, h) => h) as h}<option value={h}>{hourLabel(h)}</option>{/each}
            </select>
          {/if}
          <button
            class="inline-flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 hover:border-mute disabled:opacity-50"
            disabled={!s.elevated || busy !== null || !hoursChanged || (mode === "manual" && !spanOk)}
            onclick={saveHours}
          >
            {#if busy === "hours"}<LoaderCircle size={13} class="animate-spin" />{/if} {t("updates.hours_save")}
          </button>
          {#if mode === "manual" && !spanOk}
            <span class="text-[11.5px] text-moderate">{t("updates.hours_too_long", { max: MAX_SPAN })}</span>
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}
