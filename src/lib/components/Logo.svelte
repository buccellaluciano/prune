<script lang="ts">
  /** Logo de Prune: el mismo monograma que el ícono de la app (src-tauri/icons/prune.svg). */
  let { size = 28 }: { size?: number } = $props();
</script>

<svg viewBox="0 0 1024 1024" width={size} height={size} aria-hidden="true" class="shrink-0">
  <rect x="48" y="48" width="928" height="928" rx="208" fill="#1b1b1a" />
  <rect x="56" y="56" width="912" height="912" rx="200" fill="none" stroke="#2f2e2b" stroke-width="16" />
  <path
    d="M347 774 V250 H507 A170 170 0 0 1 507 590 H347"
    fill="none"
    stroke="#8fb5a0"
    stroke-width="104"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>
