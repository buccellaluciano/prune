<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import Logo from "./Logo.svelte";
  import { theme } from "#lib/theme.svelte.ts";
</script>

<!-- Barra de título propia del tema Moderno cuando la transparencia real está activa:
     la de Windows no deja ver el acrílico. Arrastrable; doble clic maximiza. -->
<header class="modern-titlebar flex h-8 shrink-0 items-center select-none" data-tauri-drag-region>
  <span class="pointer-events-none flex items-center gap-2 pl-3 text-[12px] text-mute" data-tauri-drag-region>
    <Logo size={16} /> P-Rune
  </span>

  <div class="ml-auto flex h-full">
    <button class="cap" onclick={() => theme.window("minimize")} aria-label={t("window.minimize")} title={t("window.minimize")}>
      <svg viewBox="0 0 10 10" width="10" height="10"><path d="M0 5h10" /></svg>
    </button>
    <button
      class="cap"
      onclick={() => theme.window("toggleMaximize")}
      aria-label={theme.maximized ? t("window.restore") : t("window.maximize")}
      title={theme.maximized ? t("window.restore") : t("window.maximize")}
    >
      {#if theme.maximized}
        <svg viewBox="0 0 10 10" width="10" height="10"><path d="M2.5 2.5V.5h7v7h-2M.5 2.5h7v7h-7z" /></svg>
      {:else}
        <svg viewBox="0 0 10 10" width="10" height="10"><path d="M.5 .5h9v9h-9z" /></svg>
      {/if}
    </button>
    <button class="cap close" onclick={() => theme.window("close")} aria-label={t("common.close")} title={t("common.close")}>
      <svg viewBox="0 0 10 10" width="10" height="10"><path d="M.5 .5l9 9M9.5 .5l-9 9" /></svg>
    </button>
  </div>
</header>

<style>
  /* Botones como los de Windows 11: 46 px, íconos de trazo fino, cerrar en rojo al pasar. */
  .cap {
    display: grid;
    place-items: center;
    width: 46px;
    height: 100%;
    color: var(--color-ink);
    transition: background 80ms;
  }
  .cap svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
  }
  .cap:hover {
    background: rgb(255 255 255 / 0.08);
  }
  .cap:active {
    background: rgb(255 255 255 / 0.05);
  }
  .cap.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  .cap.close:active {
    background: #c42b1ccc;
  }
</style>
