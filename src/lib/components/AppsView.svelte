<script lang="ts">
  import { ArrowUpCircle, Check, Download, LoaderCircle, RefreshCw, Trash2, X } from "@lucide/svelte";
  import { apps } from "$lib/apps.svelte";
  import { normalize, store } from "$lib/store.svelte";
  import { onMount } from "svelte";

  type Filter = "all" | "installed" | "updates";
  let filter = $state<Filter>("all");
  let confirmUninstall = $state<string | null>(null);

  onMount(() => {
    if (!apps.overview) apps.load();
  });

  const list = $derived.by(() => {
    const q = normalize(store.query);
    return (apps.overview?.apps ?? []).filter((a) => {
      if (filter === "installed" && !a.installed) return false;
      if (filter === "updates" && !a.available) return false;
      return !q || normalize(`${a.title} ${a.description} ${a.id}`).includes(q);
    });
  });

  const installedCount = $derived(apps.overview?.apps.filter((a) => a.installed).length ?? 0);

  function updateAll() {
    for (const a of apps.upgradable) apps.enqueue(a.id, "upgrade");
  }
</script>

<div class="mx-auto max-w-5xl">
  <header class="mb-5 flex flex-wrap items-end gap-4">
    <div class="min-w-0 flex-1">
      <h1 class="font-display text-[24px] font-semibold tracking-tight">Programas</h1>
      <p class="mt-1 max-w-2xl text-[13px] text-mute">
        Instalá y mantené al día programas confiables con winget, desde el repositorio oficial de Microsoft.
        Instalar acepta la licencia del programa; algunos instaladores piden permiso de administrador.
      </p>
    </div>
    <div class="flex items-center gap-2">
      {#if apps.upgradable.length}
        <button
          class="accent-grad inline-flex items-center gap-2 rounded-xl px-3.5 py-1.5 text-[13px] font-medium text-white shadow-lg shadow-accent/30 hover:brightness-110"
          onclick={updateAll}
        >
          <ArrowUpCircle size={15} /> Actualizar todo ({apps.upgradable.length})
        </button>
      {/if}
      <button
        class="inline-flex items-center gap-2 rounded-xl border border-line px-3 py-1.5 text-[13px] text-mute hover:bg-card-hi hover:text-ink disabled:opacity-50"
        disabled={apps.loading || !!apps.running}
        onclick={() => apps.load()}
        title="Volver a consultar winget"
      >
        <RefreshCw size={14} class={apps.loading ? "animate-spin" : ""} /> Actualizar lista
      </button>
    </div>
  </header>

  {#if apps.overview?.error && !apps.overview.apps.length}
    <p class="rounded-xl bg-advanced/10 p-4 text-[13px] text-advanced">{apps.overview.error}</p>
  {:else if !apps.overview}
    <div class="flex items-center gap-2 text-[13px] text-mute">
      <LoaderCircle size={16} class="animate-spin" /> Consultando winget… (la primera vez puede tardar unos segundos)
    </div>
  {:else}
    <div class="mb-5 flex items-center gap-1 text-[12.5px]">
      {#each [["all", "Todos", apps.overview.apps.length], ["installed", "Instalados", installedCount], ["updates", "Con actualización", apps.upgradable.length]] as [id, label, n] (id)}
        <button
          class="rounded-lg px-3 py-1.5 {filter === id ? 'bg-accent/15 text-ink' : 'text-mute hover:bg-card-hi hover:text-ink'}"
          onclick={() => (filter = id as Filter)}
        >
          {label} <span class="ml-1 font-mono text-[11px] text-faint">{n}</span>
        </button>
      {/each}
      <span class="ml-auto font-mono text-[11px] text-faint">winget {apps.overview.winget}</span>
    </div>

    {#each apps.overview.groups as g (g.id)}
      {@const items = list.filter((a) => a.group === g.id)}
      {#if items.length}
        <section class="mb-6">
          <h2 class="mb-2 text-[13px] font-semibold text-mute">{g.title}</h2>
          <div class="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-2.5">
            {#each items as a (a.id)}
              {@const job = apps.jobFor(a.id)}
              <article
                class="flex flex-col rounded-2xl border bg-card p-3.5 transition-colors
                  {job === 'running' ? 'border-accent/70' : 'border-line hover:border-[#323850]'}"
              >
                <div class="flex items-start gap-2">
                  <div class="min-w-0 flex-1">
                    <h3 class="truncate text-[14px] font-medium">{a.title}</h3>
                    <p class="mt-0.5 line-clamp-2 text-[12px] leading-relaxed text-mute">{a.description}</p>
                  </div>
                  {#if a.installed}
                    <span
                      class="shrink-0 rounded-md px-1.5 py-0.5 font-mono text-[10.5px]
                        {a.available ? 'bg-moderate/15 text-moderate' : 'bg-safe/15 text-safe'}"
                      title={a.available ? `Hay una versión nueva: ${a.available}` : "Instalado"}
                    >
                      {a.installed}{#if a.available}&nbsp;→ {a.available}{/if}
                    </span>
                  {/if}
                </div>

                <div class="mt-3 flex min-h-8 items-center gap-2">
                  {#if job === "running" && apps.running}
                    <LoaderCircle size={14} class="shrink-0 animate-spin text-accent" />
                    <span class="min-w-0 flex-1 truncate font-mono text-[11px] text-mute" title={apps.running.line}>
                      {apps.running.line || `${apps.verb(apps.running.action)}…`}
                    </span>
                  {:else if job === "queued"}
                    <span class="flex-1 text-[12px] text-faint">En cola…</span>
                    <button class="rounded-lg p-1 text-faint hover:bg-card-hi hover:text-ink" onclick={() => apps.cancelQueued(a.id)} aria-label="Sacar de la cola">
                      <X size={14} />
                    </button>
                  {:else if confirmUninstall === a.id}
                    <span class="flex-1 text-[12px] text-moderate">¿Desinstalar {a.title}?</span>
                    <button class="rounded-lg px-2 py-1 text-[12px] text-mute hover:bg-card-hi" onclick={() => (confirmUninstall = null)}>No</button>
                    <button
                      class="rounded-lg bg-advanced px-2.5 py-1 text-[12px] font-medium text-white"
                      onclick={() => { apps.enqueue(a.id, "uninstall"); confirmUninstall = null; }}
                    >
                      Sí, desinstalar
                    </button>
                  {:else}
                    <span class="flex-1 truncate font-mono text-[10.5px] text-faint">{a.id}</span>
                    {#if !a.installed}
                      <button
                        class="inline-flex items-center gap-1.5 rounded-lg bg-card-hi px-2.5 py-1 text-[12px] font-medium hover:bg-accent/20"
                        onclick={() => apps.enqueue(a.id, "install")}
                      >
                        <Download size={13} /> Instalar
                      </button>
                    {:else}
                      {#if a.available}
                        <button
                          class="inline-flex items-center gap-1.5 rounded-lg bg-moderate/15 px-2.5 py-1 text-[12px] font-medium text-moderate hover:bg-moderate/25"
                          onclick={() => apps.enqueue(a.id, "upgrade")}
                        >
                          <ArrowUpCircle size={13} /> Actualizar
                        </button>
                      {:else}
                        <span class="inline-flex items-center gap-1 text-[11.5px] text-safe"><Check size={13} /> Al día</span>
                      {/if}
                      <button
                        class="rounded-lg p-1.5 text-faint hover:bg-advanced/15 hover:text-advanced"
                        onclick={() => (confirmUninstall = a.id)}
                        title="Desinstalar"
                        aria-label="Desinstalar {a.title}"
                      >
                        <Trash2 size={14} />
                      </button>
                    {/if}
                  {/if}
                </div>
              </article>
            {/each}
          </div>
        </section>
      {/if}
    {:else}
      <p class="text-[13px] text-mute">Nada para mostrar.</p>
    {/each}
    {#if !list.length}
      <p class="text-[13px] text-mute">Nada coincide con el filtro.</p>
    {/if}
  {/if}
</div>
