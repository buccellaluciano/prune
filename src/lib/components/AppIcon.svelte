<script lang="ts">
  import { apps } from "$lib/apps.svelte";

  let { id, title, size = 28 }: { id: string; title: string; size?: number } = $props();
  const src = $derived(apps.icons[id]);
  let broken = $state(false);
</script>

<!-- Ícono del programa (del sitio oficial, en caché) o su inicial si no hay. -->
<span
  class="grid shrink-0 place-items-center overflow-hidden rounded-md border border-line bg-card-hi"
  style="width:{size}px;height:{size}px"
  aria-hidden="true"
>
  {#if src && !broken}
    <img {src} alt="" class="size-full object-contain p-[12%]" onerror={() => (broken = true)} />
  {:else}
    <span class="font-semibold text-mute" style="font-size:{Math.round(size * 0.42)}px">{title.slice(0, 1).toUpperCase()}</span>
  {/if}
</span>
