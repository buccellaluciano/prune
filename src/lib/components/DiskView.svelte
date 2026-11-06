<script lang="ts">
  import { ChevronRight, CloudOff, FolderOpen, HardDrive, Link2, Lock, LoaderCircle } from "@lucide/svelte";
  import { disk } from "$lib/disk.svelte";
  import { bytes, count, seconds } from "$lib/format";
  import { store } from "$lib/store.svelte";
  import { onMount } from "svelte";

  onMount(() => disk.init());

  const running = $derived(disk.status?.running ?? false);
  const maxHotspot = $derived(disk.summary?.hotspots[0]?.size ?? 1);

  /** Separa la ruta en "carpeta contenedora" y "nombre" para resaltar el final. */
  function split(path: string): [string, string] {
    const i = path.lastIndexOf("\\");
    return i < 0 ? ["", path] : [path.slice(0, i + 1), path.slice(i + 1)];
  }

  function pct(part: number, whole: number) {
    return whole ? Math.max(0.5, (part / whole) * 100) : 0;
  }
</script>

<div class="mx-auto max-w-4xl">
  <header class="mb-6">
    <h1 class="font-display text-[20px] font-semibold">Disco</h1>
    <p class="mt-1 max-w-2xl text-[13px] text-mute">
      Encontrá qué carpetas ocupan más espacio. Solo lectura: mide y ordena, no mueve ni borra nada.
    </p>
  </header>

  <!-- Unidades -->
  <div class="mb-8 divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
    {#each disk.drives ?? [] as d (d.letter)}
      {@const used = d.total - d.free}
      {@const scanned = disk.summary?.root === `${d.letter}:\\`}
      <div class="flex items-center gap-4 px-4 py-3">
        <HardDrive size={16} strokeWidth={1.7} class="shrink-0 text-faint" />
        <div class="min-w-0 flex-1">
          <div class="flex items-baseline gap-2 text-[13.5px]">
            <span class="font-medium">{d.letter}:</span>
            <span class="truncate text-mute">{d.label || (d.removable ? "Extraíble" : "Disco local")}</span>
            <span class="font-mono text-[10.5px] text-faint">{d.fs}</span>
          </div>
          <div class="mt-1.5 h-[3px] max-w-sm overflow-hidden rounded-full bg-line">
            <div class="h-full rounded-full {d.free / d.total < 0.1 ? 'bg-advanced' : 'bg-mute'}" style="width:{pct(used, d.total)}%"></div>
          </div>
          <div class="mt-1 text-[11.5px] text-faint">
            {bytes(used)} usados de {bytes(d.total)} · {bytes(d.free)} libres
          </div>
        </div>
        <button
          class="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:border-[#3a3935] hover:bg-line disabled:opacity-40"
          disabled={running}
          onclick={() => disk.scan(d.letter)}
        >
          {scanned ? "Volver a analizar" : "Analizar"}
        </button>
      </div>
    {:else}
      <div class="flex items-center gap-2 px-4 py-3 text-[13px] text-mute">
        <LoaderCircle size={14} class="animate-spin" /> Buscando unidades…
      </div>
    {/each}
  </div>

  {#if running && disk.status}
    {@const s = disk.status}
    <div class="rounded-lg border border-line bg-card px-4 py-3.5">
      <div class="flex items-center gap-3">
        <LoaderCircle size={15} class="animate-spin text-accent" />
        <div class="flex-1 text-[13px]">
          Analizando <span class="font-medium">{s.root}</span>
          <span class="ml-2 font-mono text-[11.5px] text-mute">
            {count(s.files)} archivos · {count(s.dirs)} carpetas · {bytes(s.bytes)}
          </span>
        </div>
        <button class="rounded-md px-2.5 py-1 text-[12px] text-mute hover:bg-line hover:text-ink" onclick={() => disk.cancel()}>
          Cancelar
        </button>
      </div>
      <p class="mt-2 truncate font-mono text-[11px] text-faint" title={s.current}>{s.current || "…"}</p>
    </div>
  {:else if disk.summary && disk.dir}
    {@const sm = disk.summary}
    <p class="mb-4 text-[12.5px] text-mute">
      <span class="font-medium text-ink">{sm.root}</span> ·
      {bytes(sm.size)} en {count(sm.files)} archivos y {count(sm.dirs)} carpetas · {seconds(sm.elapsed_ms)}
      {#if sm.cloud > 0}<br /><span class="text-faint">{bytes(sm.cloud)} más están solo en la nube (OneDrive) y no ocupan espacio local.</span>{/if}
      {#if sm.denied_dirs > 0}<br /><span class="text-faint">{count(sm.denied_dirs)} carpetas sin acceso no se pudieron medir{store.overview?.system.elevated ? "" : "; como administrador se miden casi todas"}.</span>{/if}
    </p>

    <div class="mb-5 flex items-center gap-4 border-b border-line text-[12.5px]">
      <button
        class="-mb-px border-b py-2 {disk.tab === 'hotspots' ? 'border-ink text-ink' : 'border-transparent text-mute hover:text-ink'}"
        onclick={() => (disk.tab = "hotspots")}
      >
        Dónde está el peso
      </button>
      <button
        class="-mb-px border-b py-2 {disk.tab === 'tree' ? 'border-ink text-ink' : 'border-transparent text-mute hover:text-ink'}"
        onclick={() => (disk.tab = "tree")}
      >
        Explorar
      </button>
    </div>

    {#if disk.tab === "hotspots"}
      <p class="mb-3 text-[12px] text-faint">
        Carpetas pesadas donde ninguna subcarpeta concentra más de la mitad: el lugar concreto, no sus carpetas padre.
      </p>
      <div class="divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
        {#each sm.hotspots as h (h.id)}
          {@const [parent, name] = split(h.path)}
          <div class="group flex items-center gap-4 px-4 py-2.5 hover:bg-card-hi">
            <button class="min-w-0 flex-1 text-left" onclick={() => disk.go(h.id)} title="Ver en el árbol">
              <div class="truncate text-[13px]">
                <span class="text-faint">{parent}</span><span class="font-medium text-ink">{name}</span>
              </div>
              <div class="mt-1.5 h-[3px] overflow-hidden rounded-full bg-line">
                <div class="h-full rounded-full bg-accent/70" style="width:{pct(h.size, maxHotspot)}%"></div>
              </div>
            </button>
            <span class="w-20 shrink-0 text-right font-mono text-[12px]">{bytes(h.size)}</span>
            <span class="w-24 shrink-0 text-right font-mono text-[11px] text-faint">{count(h.files)} arch.</span>
            <button class="rounded p-1.5 text-faint hover:bg-line hover:text-ink" onclick={() => disk.open(h.id)} title="Abrir en el Explorador" aria-label="Abrir {h.path} en el Explorador">
              <FolderOpen size={14} />
            </button>
          </div>
        {:else}
          <p class="px-4 py-3 text-[13px] text-mute">No hay carpetas que concentren peso.</p>
        {/each}
      </div>
    {:else}
      {@const d = disk.dir}
      <div class="mb-3 flex items-center gap-2">
        <nav class="flex min-w-0 flex-1 flex-wrap items-center gap-0.5 text-[12.5px]" aria-label="Ruta">
          {#each d.crumbs as [id, name], i (id)}
            {#if i > 0}<ChevronRight size={12} class="text-faint" />{/if}
            <button
              class="rounded px-1 py-0.5 {i === d.crumbs.length - 1 ? 'font-medium text-ink' : 'text-mute hover:bg-line hover:text-ink'}"
              onclick={() => disk.go(id)}
            >
              {i === 0 ? `${name}\\` : name}
            </button>
          {/each}
        </nav>
        <span class="font-mono text-[12px] text-mute">{bytes(d.size)}</span>
        <button class="inline-flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-[12px] text-mute hover:text-ink" onclick={() => disk.open(d.id)}>
          <FolderOpen size={13} /> Abrir
        </button>
      </div>

      <div class="divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
        {#each d.children as c (c.id)}
          <button
            class="flex w-full items-center gap-4 px-4 py-2 text-left hover:bg-card-hi disabled:cursor-default disabled:hover:bg-transparent"
            disabled={!c.has_children}
            onclick={() => disk.go(c.id)}
          >
            <span class="flex min-w-0 flex-1 items-center gap-1.5">
              <span class="truncate text-[13px] {c.size === 0 ? 'text-faint' : ''}">{c.name}</span>
              {#if c.link}<span title="Unión o enlace: no se recorre"><Link2 size={12} class="shrink-0 text-faint" /></span>{/if}
              {#if c.denied}<span title="Sin acceso"><Lock size={12} class="shrink-0 text-moderate" /></span>{/if}
              {#if c.cloud > 0}<span title="{bytes(c.cloud)} solo en la nube"><CloudOff size={12} class="shrink-0 text-faint" /></span>{/if}
            </span>
            <span class="h-[3px] w-40 shrink-0 overflow-hidden rounded-full bg-line">
              <span class="block h-full rounded-full bg-accent/70" style="width:{pct(c.size, d.size)}%"></span>
            </span>
            <span class="w-12 shrink-0 text-right font-mono text-[11px] text-faint">
              {d.size ? Math.round((c.size / d.size) * 100) : 0}%
            </span>
            <span class="w-20 shrink-0 text-right font-mono text-[12px]">{bytes(c.size)}</span>
            <ChevronRight size={13} class="shrink-0 {c.has_children ? 'text-faint' : 'invisible'}" />
          </button>
        {/each}
        {#if d.own > 0}
          <div class="flex items-center gap-4 px-4 py-2 text-faint">
            <span class="flex-1 text-[13px] italic">Archivos sueltos en esta carpeta ({count(d.own_files)})</span>
            <span class="h-[3px] w-40 shrink-0 overflow-hidden rounded-full bg-line">
              <span class="block h-full rounded-full bg-mute/60" style="width:{pct(d.own, d.size)}%"></span>
            </span>
            <span class="w-12 shrink-0 text-right font-mono text-[11px]">{Math.round((d.own / d.size) * 100)}%</span>
            <span class="w-20 shrink-0 text-right font-mono text-[12px]">{bytes(d.own)}</span>
            <span class="w-[13px]"></span>
          </div>
        {/if}
        {#if d.hidden > 0}
          <p class="px-4 py-2 text-[12px] text-faint">…y {count(d.hidden)} carpetas más chicas.</p>
        {/if}
      </div>
    {/if}
  {:else if disk.status?.error}
    <p class="rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{disk.status.error}</p>
  {/if}
</div>
