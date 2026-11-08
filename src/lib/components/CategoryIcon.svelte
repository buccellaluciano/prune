<script lang="ts">
  import {
    Shield, Sparkles, MegaphoneOff, Search, PanelBottom, Folder, Palette, Gamepad2, Zap, Cog,
  } from "@lucide/svelte";

  let { name, size = 16 }: { name: string; size?: number } = $props();

  const map: Record<string, typeof Shield> = {
    shield: Shield,
    sparkles: Sparkles,
    "megaphone-off": MegaphoneOff,
    search: Search,
    "panel-bottom": PanelBottom,
    folder: Folder,
    palette: Palette,
    gamepad: Gamepad2,
    cog: Cog,
  };
  const Icon = $derived(map[name] ?? Zap);
</script>

<Icon {size} strokeWidth={1.8} />
