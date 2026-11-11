<script lang="ts">
  /** Inicio de Windows: lo que arranca al iniciar sesión, con un interruptor por entrada. */
  import { LoaderCircle, RefreshCw, Rocket, ShieldAlert, TriangleAlert } from "@lucide/svelte";
  import { startupApi, type StartupItem, type StartupSource, type StartupStatus } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import { t, type Key } from "#lib/i18n.svelte.ts";
  import { onMount } from "svelte";

  let s = $state<StartupStatus | null>(null);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);

  async function load() {
    try {
      s = await startupApi.status();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);

  const SOURCE: Record<StartupSource, Key> = {
    run_user: "startup.source.run_user",
    run_machine: "startup.source.run_machine",
    run_machine32: "startup.source.run_machine32",
    folder_user: "startup.source.folder_user",
    folder_machine: "startup.source.folder_machine",
    task: "startup.source.task",
  };

  const key = (i: StartupItem) => `${i.source}|${i.name}`;
  const perUser = (i: StartupItem) => i.source === "run_user" || i.source === "folder_user";
  const blocked = (i: StartupItem) => (!perUser(i) && !s?.elevated ? t("disabled.admin") : null);
  const on = $derived(s?.items.filter((i) => i.enabled).length ?? 0);

  async function toggle(i: StartupItem) {
    busy = key(i);
    const enabled = !i.enabled;
    try {
      const id = await startupApi.set(i.source, i.name, enabled);
      if (id !== null) store.toast("ok", t(enabled ? "startup.turned_on" : "startup.turned_off", { name: i.title }));
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      busy = null;
      await load();
    }
  }
</script>

<div class="mx-auto max-w-3xl">
  <div class="flex items-start gap-3">
    <div class="flex-1">
      <h1 class="font-display text-[20px] font-semibold">{t("nav.startup")}</h1>
      <p class="mt-1 mb-5 text-[13px] text-mute">{t("startup.subtitle")}</p>
    </div>
    <button
      class="mt-1 rounded-md p-1.5 text-mute hover:bg-card-hi hover:text-ink disabled:opacity-50"
      title={t("startup.refresh")}
      aria-label={t("startup.refresh")}
      disabled={!s || busy !== null}
      onclick={() => ((s = null), load())}
    >
      <RefreshCw size={15} />
    </button>
  </div>

  {#if error}
    <p class="rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{error}</p>
  {:else if !s}
    <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> {t("startup.loading")}</div>
  {:else}
    {#if s.foreign_user}
      <div class="mb-3 flex gap-2 rounded-md border border-line px-3.5 py-2.5 text-[12.5px] text-mute">
        <ShieldAlert size={15} class="mt-px shrink-0 text-moderate" /><span>{t("startup.foreign")}</span>
      </div>
    {:else if !s.elevated}
      <div class="mb-3 flex gap-2 rounded-md border border-line px-3.5 py-2.5 text-[12.5px] text-mute">
        <ShieldAlert size={15} class="mt-px shrink-0 text-moderate" /><span>{t("startup.admin_needed")}</span>
      </div>
    {/if}

    {#if s.items.length === 0}
      <div class="rounded-lg border border-line bg-card p-10 text-center text-[13px] text-mute">
        <Rocket size={26} class="mx-auto mb-3 text-faint" /> {t("startup.empty")}
      </div>
    {:else}
      <h2 class="section-title mb-2 flex items-baseline gap-2 text-[13px] font-medium">
        {t("startup.section")}
        <span class="font-mono text-[11px] font-normal text-faint">{on}/{s.items.length}</span>
      </h2>
      <div class="divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
        {#each s.items as i (key(i))}
          {@const reason = blocked(i)}
          {@const disabled = !!reason || busy !== null}
          <div class="flex items-start gap-4 p-4 {i.enabled ? '' : 'opacity-70'}">
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
                <span class="text-[13.5px] font-medium">{i.title}</span>
                <span class="text-[11px] text-faint">{t(SOURCE[i.source])}</span>
              </div>
              <div class="mt-0.5 text-[12px] text-mute">
                {#if i.system}
                  {t("startup.windows")}
                {:else if i.publisher}
                  {i.publisher}
                {:else if i.exists}
                  <span class="text-moderate">{t("startup.unsigned")}</span>
                {/if}
              </div>
              <div class="mt-1 font-mono text-[11px] break-all text-faint" title={i.command}>{i.command}</div>
              {#if i.path && !i.exists}
                <div class="mt-1.5 flex gap-1.5 text-[11.5px] text-moderate">
                  <TriangleAlert size={12} class="mt-px shrink-0" /><span>{t("startup.missing_file")}</span>
                </div>
              {:else if i.system && i.enabled}
                <div class="mt-1.5 text-[11.5px] text-faint">{t("startup.windows_hint")}</div>
              {/if}
            </div>
            <button
              role="switch"
              aria-checked={i.enabled}
              aria-label={i.title}
              title={reason ?? undefined}
              {disabled}
              onclick={() => toggle(i)}
              class="relative mt-0.5 h-5 w-9 shrink-0 rounded-full border transition-colors duration-150
                {i.enabled ? 'border-accent bg-accent' : 'border-[#3a3935] bg-transparent'} {disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}"
            >
              {#if busy === key(i)}
                <LoaderCircle size={12} class="absolute top-[3px] left-[11px] animate-spin {i.enabled ? 'text-bg' : 'text-mute'}" />
              {:else}
                <span class="absolute top-[3px] left-[3px] size-3 rounded-full transition-transform duration-150 {i.enabled ? 'translate-x-4 bg-bg' : 'bg-mute'}"></span>
              {/if}
            </button>
          </div>
        {/each}
      </div>
      <p class="mt-2 text-[11.5px] text-faint">{t("startup.footnote")}</p>
    {/if}
  {/if}
</div>
