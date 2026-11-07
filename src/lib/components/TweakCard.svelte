<script lang="ts">
  import { ChevronRight } from "@lucide/svelte";
  import type { Tweak } from "#lib/api.ts";
  import { store } from "#lib/store.svelte.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import { slide } from "svelte/transition";

  let { tweak }: { tweak: Tweak } = $props();
  let open = $state(false);

  const on = $derived(store.shown(tweak));
  const isChoice = $derived(tweak.choices.length > 0);
  const current = $derived(store.shownChoice(tweak));
  const isPending = $derived(tweak.id in store.pending);
  const disabled = $derived(store.disabledReason(tweak));

  const riskLabel = $derived({ safe: null, moderate: t("risk.moderate"), advanced: t("risk.advanced") });
  const riskCls = { safe: "", moderate: "text-moderate", advanced: "text-advanced" } as const;
  const restartLabel = $derived({
    none: null,
    explorer: t("restart.explorer"),
    logoff: t("restart.logoff"),
    reboot: t("restart.reboot"),
  });

  const meta = $derived(
    [
      tweak.requires_admin ? t("tweak.admin") : null,
      restartLabel[tweak.restart],
      tweak.state === "unsupported" ? t("tweak.unsupported") : null,
    ].filter(Boolean) as string[],
  );
</script>

<!-- Una fila de la lista: título y descripción a la izquierda, control a la derecha. -->
<article class="tweak-row group relative px-4 py-3 transition-colors hover:bg-card-hi {disabled ? 'opacity-55' : ''}">
  {#if isPending}
    <span class="absolute inset-y-2 left-0 w-0.5 rounded-full bg-accent" aria-hidden="true"></span>
  {/if}

  <div class="flex items-start gap-4">
    <div class="min-w-0 flex-1">
      <h3 class="text-[13.5px] leading-snug font-medium">{tweak.title}</h3>
      <p class="mt-0.5 text-[12.5px] leading-relaxed text-mute">{tweak.description}</p>

      {#if tweak.ucpd && store.overview?.system.ucpd_active}
        <p class="mt-1.5 text-[12px] leading-relaxed text-moderate">
          {t("tweak.ucpd.before")}
          <button class="underline underline-offset-2 hover:text-ink" onclick={() => ((store.view = "bloat"), (store.query = ""))}>{t("tweak.ucpd.link")}</button>{t("tweak.ucpd.after")}
        </p>
      {/if}

      <div class="mt-1.5 flex flex-wrap items-center gap-x-2 text-[11.5px] text-faint">
        {#if isPending}
          <span class="text-accent">
            {#if isChoice}
              {store.choiceLabel(tweak, tweak.choice)} → {store.choiceLabel(tweak, current)}
            {:else}
              {store.pending[tweak.id] ? t("tweak.will_apply") : t("tweak.will_revert")}
            {/if}
          </span>
          <span aria-hidden="true">·</span>
        {/if}
        {#if riskLabel[tweak.risk]}
          <span class={riskCls[tweak.risk]}>{riskLabel[tweak.risk]}</span>
          <span aria-hidden="true">·</span>
        {/if}
        {#if tweak.state === "partial"}
          <span class="text-moderate">{isChoice ? t("tweak.unrecognized") : t("tweak.partial")}</span>
          <span aria-hidden="true">·</span>
        {/if}
        {#each meta as m}
          <span>{m}</span>
          <span aria-hidden="true">·</span>
        {/each}
        <button class="inline-flex items-center gap-0.5 hover:text-ink" onclick={() => (open = !open)} aria-expanded={open}>
          {t("tweak.registry")} <ChevronRight size={11} class="transition-transform {open ? 'rotate-90' : ''}" />
        </button>
      </div>
    </div>

    {#if isChoice}
      <div
        role="radiogroup"
        aria-label={tweak.title}
        title={disabled ?? undefined}
        class="flex shrink-0 rounded-md border border-line p-0.5 {disabled ? 'cursor-not-allowed' : ''}"
      >
        {#each tweak.choices as c (c.id)}
          {@const sel = current === c.id}
          <button
            role="radio"
            aria-checked={sel}
            disabled={!!disabled}
            onclick={() => store.choose(tweak, c.id)}
            class="relative rounded-[4px] px-2.5 py-1 text-[12px] whitespace-nowrap transition-colors
              {sel ? (isPending ? 'accent-grad font-medium' : 'bg-card-hi font-medium text-ink') : 'text-mute hover:text-ink'}
              {disabled ? 'pointer-events-none' : ''}"
          >
            {c.label}
            {#if c.id === tweak.choice && isPending}
              <span class="absolute top-0.5 right-0.5 size-1 rounded-full bg-mute" title={t("tweak.current")}></span>
            {/if}
          </button>
        {/each}
      </div>
    {:else}
      <button
        role="switch"
        aria-checked={on}
        aria-label={tweak.title}
        title={disabled ?? (on ? t("tweak.on") : t("tweak.off"))}
        disabled={!!disabled}
        onclick={() => store.toggle(tweak)}
        class="relative mt-0.5 h-5 w-9 shrink-0 rounded-full border transition-colors duration-150
          {on ? 'border-accent bg-accent' : 'border-[#3a3935] bg-transparent'} {disabled ? 'cursor-not-allowed' : 'cursor-pointer'}"
      >
        <span
          class="absolute top-[3px] left-[3px] size-3 rounded-full transition-transform duration-150
            {on ? 'translate-x-4 bg-bg' : 'bg-mute'}"
        ></span>
      </button>
    {/if}
  </div>

  {#if open}
    <div transition:slide={{ duration: 140 }} class="mt-2.5 space-y-1.5">
      {#each tweak.ops as op}
        <div class="rounded-md border border-line bg-bg/60 px-2.5 py-2 font-mono text-[10.5px] leading-relaxed">
          <div class="break-all text-faint">{op.key}</div>
          <div class="mt-0.5 flex flex-wrap gap-x-3 text-mute">
            <span class="text-ink">{op.name}</span>
            <span>{t("op.current")} <b class="font-normal text-ink">{op.current}</b></span>
            {#if isChoice}
              <span>{t("op.options")} <b class="font-normal text-accent">{op.on}</b></span>
            {:else}
              <span>{t("op.on")} <b class="font-normal text-accent">{op.on}</b></span>
              <span>{t("op.default")} <b class="font-normal text-ink">{op.off}</b></span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</article>
