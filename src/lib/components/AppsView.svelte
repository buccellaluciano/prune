<script lang="ts">
  import { LoaderCircle, RefreshCw, Trash2, X } from "@lucide/svelte";
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

<div class="mx-auto max-w-4xl">
  <header class="mb-6 flex flex-wrap items-end gap-4">
    <div class="min-w-0 flex-1">
      <h1 class="font-display text-[20px] font-semibold">Programas</h1>
      <p class="mt-1 max-w-2xl text-[13px] text-mute">
        Programas confiables desde el repositorio oficial de winget. Instalar acepta la licencia del programa;
        algunos instaladores piden permiso de administrador.
      </p>
    </div>
    <div class="flex items-center gap-2">
      <button
        class="inline-flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-[12.5px] text-mute hover:text-ink disabled:opacity-50"
        disabled={apps.loading || !!apps.running}
        onclick={() => apps.load()}
        title="Volver a consultar winget"
      >
        <RefreshCw size={13} class={apps.loading ? "animate-spin" : ""} /> Recargar
      </button>
      {#if apps.upgradable.length}
        <button class="accent-grad rounded-md px-3 py-1 text-[12.5px] font-medium" onclick={updateAll}>
          Actualizar todo ({apps.upgradable.length})
        </button>
      {/if}
    </div>
  </header>

  {#if apps.overview?.error && !apps.overview.apps.length}
    <p class="rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{apps.overview.error}</p>
  {:else if !apps.overview}
    <div class="flex items-center gap-2 text-[13px] text-mute">
      <LoaderCircle size={15} class="animate-spin" /> Consultando winget… la primera vez puede tardar unos segundos.
    </div>
  {:else}
    <div class="mb-6 flex items-center gap-4 border-b border-line text-[12.5px]">
      {#each [["all", "Todos", apps.overview.apps.length], ["installed", "Instalados", installedCount], ["updates", "Con actualización", apps.upgradable.length]] as [id, label, n] (id)}
        <button
          class="-mb-px border-b py-2 {filter === id ? 'border-ink text-ink' : 'border-transparent text-mute hover:text-ink'}"
          onclick={() => (filter = id as Filter)}
        >
          {label} <span class="ml-0.5 font-mono text-[11px] text-faint">{n}</span>
        </button>
      {/each}
      <span class="ml-auto font-mono text-[11px] text-faint">winget {apps.overview.winget}</span>
    </div>

    {#each apps.overview.groups as g (g.id)}
      {@const items = list.filter((a) => a.group === g.id)}
      {#if items.length}
        <section class="mb-8">
          <h2 class="section-title mb-2 text-[13px] font-medium">{g.title}</h2>
          <div class="divide-y divide-line overflow-hidden rounded-lg border border-line bg-card">
            {#each items as a (a.id)}
              {@const job = apps.jobFor(a.id)}
              <article class="flex items-center gap-4 px-4 py-2.5 transition-colors hover:bg-card-hi">
                <div class="min-w-0 flex-1">
                  <div class="flex items-baseline gap-2">
                    <h3 class="truncate text-[13.5px] font-medium">{a.title}</h3>
                    <span class="truncate font-mono text-[10.5px] text-faint">{a.id}</span>
                  </div>
                  {#if job === "running" && apps.running}
                    <p class="truncate font-mono text-[11px] text-accent" title={apps.running.line}>
                      {apps.running.line || `${apps.verb(apps.running.action)}…`}
                    </p>
                  {:else}
                    <p class="truncate text-[12.5px] text-mute">{a.description}</p>
                  {/if}
                </div>

                <div class="w-36 shrink-0 text-right font-mono text-[11px]">
                  {#if a.installed && a.available}
                    <span class="text-faint">{a.installed}</span> <span class="text-moderate">→ {a.available}</span>
                  {:else if a.installed}
                    <span class="text-mute">{a.installed}</span>
                  {/if}
                </div>

                <div class="flex w-44 shrink-0 items-center justify-end gap-1.5">
                  {#if job === "running"}
                    <LoaderCircle size={14} class="animate-spin text-accent" />
                  {:else if job === "queued"}
                    <span class="text-[12px] text-faint">En cola</span>
                    <button class="rounded p-1 text-faint hover:bg-line hover:text-ink" onclick={() => apps.cancelQueued(a.id)} aria-label="Sacar de la cola">
                      <X size={13} />
                    </button>
                  {:else if confirmUninstall === a.id}
                    <button class="rounded-md px-2 py-1 text-[12px] text-mute hover:text-ink" onclick={() => (confirmUninstall = null)}>Cancelar</button>
                    <button
                      class="rounded-md border border-advanced/50 px-2.5 py-1 text-[12px] text-advanced hover:bg-advanced/10"
                      onclick={() => { apps.enqueue(a.id, "uninstall"); confirmUninstall = null; }}
                    >
                      Desinstalar
                    </button>
                  {:else if !a.installed}
                    <button
                      class="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:border-[#3a3935] hover:bg-line"
                      onclick={() => apps.enqueue(a.id, "install")}
                    >
                      Instalar
                    </button>
                  {:else}
                    {#if a.available}
                      <button
                        class="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:border-[#3a3935] hover:bg-line"
                        onclick={() => apps.enqueue(a.id, "upgrade")}
                      >
                        Actualizar
                      </button>
                    {/if}
                    <button
                      class="rounded p-1.5 text-faint hover:bg-line hover:text-advanced"
                      onclick={() => (confirmUninstall = a.id)}
                      title="Desinstalar"
                      aria-label="Desinstalar {a.title}"
                    >
                      <Trash2 size={13} />
                    </button>
                  {/if}
                </div>
              </article>
            {/each}
          </div>
        </section>
      {/if}
    {/each}
    {#if !list.length}
      <p class="text-[13px] text-mute">Nada coincide con el filtro.</p>
    {/if}
  {/if}
</div>
