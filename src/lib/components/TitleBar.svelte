<script lang="ts">
  import Logo from "./Logo.svelte";
  import { theme } from "$lib/theme.svelte";
</script>

<!-- Barra de título propia del tema Aero: arrastrable, doble clic maximiza. -->
<header class="aero-titlebar" data-tauri-drag-region>
  <span class="aero-titlebar-logo" data-tauri-drag-region><Logo size={16} /></span>
  <span class="aero-titlebar-text" data-tauri-drag-region>Prune</span>

  <div class="aero-caption">
    <button class="cap" onclick={() => theme.window("minimize")} aria-label="Minimizar" title="Minimizar">
      <svg viewBox="0 0 10 10" width="10" height="10"><rect x="1" y="6" width="8" height="2.4" /></svg>
    </button>
    <button
      class="cap"
      onclick={() => theme.window("toggleMaximize")}
      aria-label={theme.maximized ? "Restaurar" : "Maximizar"}
      title={theme.maximized ? "Restaurar" : "Maximizar"}
    >
      {#if theme.maximized}
        <svg viewBox="0 0 10 10" width="10" height="10">
          <path d="M3 1h6v5H7V3H3z M1 4h6v5H1z M2.4 5.6h3.2v2H2.4z" fill-rule="evenodd" />
        </svg>
      {:else}
        <svg viewBox="0 0 10 10" width="10" height="10"><path d="M1 1h8v8H1z M2.6 3.2h4.8v4.2H2.6z" fill-rule="evenodd" /></svg>
      {/if}
    </button>
    <button class="cap close" onclick={() => theme.window("close")} aria-label="Cerrar" title="Cerrar">
      <svg viewBox="0 0 10 10" width="10" height="10">
        <path d="M1.2 0.4 5 4.2 8.8 0.4 9.6 1.2 5.8 5 9.6 8.8 8.8 9.6 5 5.8 1.2 9.6 0.4 8.8 4.2 5 0.4 1.2z" />
      </svg>
    </button>
  </div>
</header>
