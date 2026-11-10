<script lang="ts">
  /** Ícono de WMO dentro de P-Rune: la "isla" (una cápsula) con una nota, en el estilo del logo. */
  let { size = 56 }: { size?: number } = $props();
</script>

<svg viewBox="0 0 1024 1024" width={size} height={size} aria-hidden="true" class="shrink-0">
  <rect x="48" y="48" width="928" height="928" rx="208" fill="#1b1b1a" />
  <rect x="56" y="56" width="912" height="912" rx="200" fill="none" stroke="#2f2e2b" stroke-width="16" />
  <rect x="192" y="372" width="640" height="280" rx="140" fill="none" stroke="#8fb5a0" stroke-width="48" />
  <path d="M452 590 V440 L572 418 V568" fill="none" stroke="#8fb5a0" stroke-width="32" stroke-linecap="round" stroke-linejoin="round" />
  <circle cx="428" cy="592" r="30" fill="#8fb5a0" />
  <circle cx="548" cy="570" r="30" fill="#8fb5a0" />
</svg>
