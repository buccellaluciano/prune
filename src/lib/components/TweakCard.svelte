<script lang="ts">
  import { ChevronRight, ShieldCheck, RotateCcw } from "@lucide/svelte";
  import type { Tweak } from "$lib/api";
  import { store } from "$lib/store.svelte";
  import { slide } from "svelte/transition";

  let { tweak }: { tweak: Tweak } = $props();
  let open = $state(false);

  const on = $derived(store.shown(tweak));
  const isChoice = $derived(tweak.choices.length > 0);
  const current = $derived(store.shownChoice(tweak));
  const isPending = $derived(tweak.id in store.pending);
  const disabled = $derived(store.disabledReason(tweak));

  const riskLabel = { safe: "Seguro", moderate: "Moderado", advanced: "Avanzado" } as const;
  const riskDot = { safe: "bg-safe", moderate: "bg-moderate", advanced: "bg-advanced" } as const;
  const restartLabel = { none: null, explorer: "Explorador", logoff: "Cerrar sesión", reboot: "Reinicio" } as const;
</script>

<article
  class="group relative rounded-2xl border bg-card p-4 transition-all duration-200
    {isPending ? 'border-accent/70 shadow-[0_0_0_1px_rgb(124_108_255/0.35),0_8px_30px_-12px_rgb(124_108_255/0.5)]' : 'border-line hover:border-[#323850] hover:bg-card-hi'}
    {disabled ? 'opacity-60' : ''}"
>
  <div class="flex items-start gap-3">
    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-2">
        <span class="size-2 shrink-0 rounded-full {riskDot[tweak.risk]}" title={riskLabel[tweak.risk]}></span>
        <h3 class="truncate text-[14px] font-medium">{tweak.title}</h3>
      </div>
      <p class="mt-1 line-clamp-2 text-[12.5px] leading-relaxed text-mute">{tweak.description}</p>
    </div>

    {#if !isChoice}
    <button
      role="switch"
      aria-checked={on}
      aria-label={tweak.title}
      title={disabled ?? (on ? "Activado" : "Desactivado")}
      disabled={!!disabled}
      onclick={() => store.toggle(tweak)}
      class="relative mt-0.5 h-6 w-11 shrink-0 rounded-full transition-colors duration-200
        {on ? 'accent-grad' : 'bg-line'} {disabled ? 'cursor-not-allowed' : 'cursor-pointer'}"
    >
      <span
        class="absolute top-0.5 left-0.5 size-5 rounded-full bg-white shadow transition-transform duration-200
          {on ? 'translate-x-5' : ''}"
      ></span>
    </button>
    {/if}
  </div>

  {#if isChoice}
    <div
      role="radiogroup"
      aria-label={tweak.title}
      title={disabled ?? undefined}
      class="mt-3 flex rounded-xl border border-line bg-bg/60 p-0.5 {disabled ? 'cursor-not-allowed' : ''}"
    >
      {#each tweak.choices as c (c.id)}
        {@const sel = current === c.id}
        <button
          role="radio"
          aria-checked={sel}
          disabled={!!disabled}
          onclick={() => store.choose(tweak, c.id)}
          class="relative flex-1 rounded-[10px] px-2 py-1.5 text-[12px] whitespace-nowrap transition-colors
            {sel ? (isPending ? 'accent-grad font-medium text-white shadow' : 'bg-card-hi font-medium text-ink shadow') : 'text-mute hover:text-ink'}
            {disabled ? 'pointer-events-none' : ''}"
        >
          {c.label}
          {#if c.id === tweak.choice && isPending}
            <span class="absolute top-1 right-1 size-1.5 rounded-full bg-mute" title="Actual"></span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}

  <div class="mt-3 flex flex-wrap items-center gap-1.5 text-[11px]">
    {#if isPending}
      <span class="rounded-md bg-accent/20 px-1.5 py-0.5 font-medium text-[#b9b0ff]">
        {#if isChoice}
          {store.choiceLabel(tweak, tweak.choice)} → {store.choiceLabel(tweak, current)}
        {:else}
          {store.pending[tweak.id] ? "se aplicará" : "se revertirá"}
        {/if}
      </span>
    {/if}
    {#if tweak.state === "partial"}
      <span class="rounded-md bg-moderate/15 px-1.5 py-0.5 text-moderate">{isChoice ? "valor no reconocido" : "parcial"}</span>
    {/if}
    {#if tweak.state === "unsupported"}
      <span class="rounded-md bg-line px-1.5 py-0.5 text-mute">no aplica a tu build</span>
    {/if}
    {#if tweak.requires_admin}
      <span class="inline-flex items-center gap-1 rounded-md bg-line/70 px-1.5 py-0.5 text-mute">
        <ShieldCheck size={11} /> admin
      </span>
    {/if}
    {#if restartLabel[tweak.restart]}
      <span class="inline-flex items-center gap-1 rounded-md bg-line/70 px-1.5 py-0.5 text-mute">
        <RotateCcw size={11} /> {restartLabel[tweak.restart]}
      </span>
    {/if}
    <button
      class="ml-auto inline-flex items-center gap-0.5 text-faint hover:text-ink"
      onclick={() => (open = !open)}
      aria-expanded={open}
    >
      registro <ChevronRight size={12} class="transition-transform {open ? 'rotate-90' : ''}" />
    </button>
  </div>

  {#if open}
    <div transition:slide={{ duration: 160 }} class="mt-3 space-y-2 border-t border-line pt-3">
      {#each tweak.ops as op}
        <div class="rounded-lg bg-bg/60 p-2 font-mono text-[10.5px] leading-relaxed">
          <div class="break-all text-faint">{op.key}</div>
          <div class="mt-0.5 flex flex-wrap gap-x-3 text-mute">
            <span class="text-ink">{op.name}</span>
            <span>actual <b class="font-normal text-ink">{op.current}</b></span>
            {#if isChoice}
              <span>opciones <b class="font-normal text-safe">{op.on}</b></span>
            {:else}
              <span>activo <b class="font-normal text-safe">{op.on}</b></span>
              <span>por defecto <b class="font-normal text-ink">{op.off}</b></span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</article>
