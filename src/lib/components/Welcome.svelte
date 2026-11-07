<script lang="ts">
  /**
   * Bienvenida al abrir la app: foto y cuenta de Windows de quien la abrió.
   * Dura lo mínimo para verse (~1,2 s) y, si el estado del sistema todavía se está leyendo,
   * lo espera hasta un tope, así tapa la carga inicial en vez de sumarle tiempo.
   * Se saltea con un clic o cualquier tecla.
   */
  import { onMount } from "svelte";
  import Logo from "./Logo.svelte";
  import { store } from "#lib/store.svelte.ts";
  import { initials } from "#lib/format.ts";
  import { t } from "#lib/i18n.svelte.ts";

  const MIN_MS = 1200;
  const MAX_MS = 3000;
  const OUT_MS = 320;

  let visible = $state(true);
  let leaving = $state(false);
  let minElapsed = $state(false);

  const u = $derived(store.user);
  const ready = $derived(store.userLoaded && !store.loading);

  function leave() {
    if (leaving) return;
    leaving = true;
    setTimeout(() => (visible = false), OUT_MS);
  }

  $effect(() => {
    if (minElapsed && ready) leave();
  });

  onMount(() => {
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const a = setTimeout(() => (minElapsed = true), reduced ? 500 : MIN_MS);
    const b = setTimeout(leave, MAX_MS);
    return () => (clearTimeout(a), clearTimeout(b));
  });
</script>

<svelte:window onkeydown={() => visible && leave()} />

{#if visible}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="welcome fixed inset-0 z-[100] grid place-items-center bg-bg"
    class:leaving
    role="presentation"
    onclick={leave}
  >
    <div class="flex flex-col items-center text-center">
      <div class="avatar-wrap relative size-[104px]">
        <svg class="arc absolute inset-0" viewBox="0 0 104 104" aria-hidden="true">
          <circle cx="52" cy="52" r="50" />
        </svg>
        <div class="avatar absolute inset-[8px] overflow-hidden rounded-full">
          {#if u?.picture}
            <img src={u.picture} alt="" class="size-full object-cover" />
          {:else if u}
            <div class="grid size-full place-items-center bg-card-hi text-[30px] font-medium text-ink uppercase">
              {initials(u.name || u.username)}
            </div>
          {:else}
            <div class="size-full bg-card-hi"></div>
          {/if}
        </div>
      </div>

      <div class="greet mt-6 font-display text-[24px] font-semibold text-ink">
        {u ? t("welcome.hello_name", { name: u.name || u.username }) : t("welcome.hello")}
      </div>
      {#if u}
        <div class="who mt-1.5 font-mono text-[12px] text-faint">{u.username}</div>
      {/if}
    </div>

    <div class="brand absolute bottom-8 flex items-center gap-2 text-[12px] text-faint">
      <Logo size={16} /> P-Rune
    </div>
  </div>
{/if}

<style>
  .welcome {
    transition: opacity 320ms ease, transform 320ms ease;
  }
  .welcome.leaving {
    opacity: 0;
    transform: scale(1.015);
    pointer-events: none;
  }

  /* El aro se dibuja alrededor de la foto con el verde del logo. */
  .arc circle {
    fill: none;
    stroke: var(--color-accent);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-dasharray: 315;
    stroke-dashoffset: 315;
    transform: rotate(-90deg);
    transform-origin: center;
    animation: draw 900ms cubic-bezier(0.65, 0, 0.35, 1) 80ms forwards;
  }
  .avatar {
    opacity: 0;
    transform: scale(0.86);
    animation: pop 520ms cubic-bezier(0.2, 0.9, 0.3, 1.15) 120ms forwards;
  }
  .greet,
  .who,
  .brand {
    opacity: 0;
    transform: translateY(6px);
    animation: rise 420ms ease-out forwards;
  }
  .greet {
    animation-delay: 300ms;
  }
  .who {
    animation-delay: 400ms;
  }
  .brand {
    animation-delay: 520ms;
  }

  @keyframes draw {
    to {
      stroke-dashoffset: 0;
    }
  }
  @keyframes pop {
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
  @keyframes rise {
    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .arc circle,
    .avatar,
    .greet,
    .who,
    .brand {
      animation: none;
      opacity: 1;
      transform: none;
      stroke-dashoffset: 0;
    }
    .welcome {
      transition: opacity 150ms;
    }
  }
</style>
