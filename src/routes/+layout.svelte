<script lang="ts">
  import "../app.css";
  let { children } = $props();

  /**
   * Sin menú contextual del navegador (Recargar, Inspeccionar, Guardar como…) en la app.
   * Se deja solo en campos de texto, para poder copiar y pegar.
   */
  function oncontextmenu(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    const editable = t?.closest("input, textarea, [contenteditable='true']");
    if (!editable) e.preventDefault();
  }
</script>

<svelte:document {oncontextmenu} />

{@render children()}
