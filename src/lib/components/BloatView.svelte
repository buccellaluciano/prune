<script lang="ts">
  import { LoaderCircle, PackageX, TriangleAlert } from "@lucide/svelte";
  import { api, type BloatApp, type Risk } from "$lib/api";
  import { store } from "$lib/store.svelte";
  import { onMount } from "svelte";

  let apps = $state<BloatApp[] | null>(null);
  let error = $state<string | null>(null);
  let selected = $state<Record<string, boolean>>({});
  let deprovision = $state(true);
  let confirming = $state(false);
  let working = $state(false);

  const tiers: { id: Risk; title: string; hint: string; cls: string }[] = [
    { id: "safe", title: "Seguro", hint: "Nadie lo extraña", cls: "text-safe" },
    { id: "moderate", title: "Moderado", hint: "Puede que lo uses", cls: "text-moderate" },
    { id: "advanced", title: "Avanzado", hint: "Rompe funciones: leé la nota", cls: "text-advanced" },
  ];

  const chosen = $derived(Object.keys(selected).filter((k) => selected[k]));
  const elevated = $derived(store.overview?.system.elevated ?? false);

  async function load() {
    apps = null;
    error = null;
    try {
      apps = await api.listBloat();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);

  function selectTier(t: Risk) {
    const copy = { ...selected };
    for (const a of apps ?? []) if (a.tier === t) copy[a.id] = true;
    selected = copy;
  }

  async function remove() {
    working = true;
    try {
      const res = await api.removeBloat(chosen, deprovision);
      const ok = res.filter((r) => r.ok).length;
      if (ok) store.toast("ok", `${ok} ${ok === 1 ? "app quitada" : "apps quitadas"}.`);
      for (const r of res.filter((r) => !r.ok)) store.toast("error", `${r.id}: ${r.error}`);
      selected = {};
      confirming = false;
      await load();
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      working = false;
    }
  }
</script>

<div class="mx-auto max-w-5xl">
  <header class="mb-5 flex items-end gap-4">
    <div class="flex-1">
      <h1 class="font-display text-[20px] font-semibold">Bloatware</h1>
      <p class="mt-1 max-w-2xl text-[13px] text-mute">
        Solo se muestran las apps del catálogo que tenés instaladas. Lo esencial (Store, winget, Terminal, Calculadora,
        Seguridad de Windows) nunca aparece acá. Quitar una app <b class="text-ink">no se puede deshacer</b> desde esta app:
        se reinstala desde Microsoft Store.
      </p>
    </div>
  </header>

  {#if error}
    <p class="rounded-md border border-advanced/30 p-4 text-[13px] text-advanced">{error}</p>
  {:else if !apps}
    <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> Buscando paquetes instalados…</div>
  {:else if apps.length === 0}
    <div class="rounded-lg border border-line bg-card p-10 text-center">
      <PackageX size={28} class="mx-auto text-safe" />
      <p class="mt-3 text-[14px]">No quedó bloatware del catálogo instalado.</p>
    </div>
  {:else}
    {#each tiers as tier (tier.id)}
      {@const list = apps.filter((a) => a.tier === tier.id)}
      {#if list.length}
        <section class="mb-6">
          <div class="mb-2 flex items-baseline gap-3">
            <h2 class="section-title text-[13px] font-medium {tier.cls}">{tier.title}</h2>
            <span class="text-[12px] text-faint">{tier.hint}</span>
            <button class="ml-auto text-[12px] text-mute hover:text-ink" onclick={() => selectTier(tier.id)}>Seleccionar todo</button>
          </div>
          <div class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-2">
            {#each list as app (app.id)}
              <label
                class="flex cursor-pointer gap-3 rounded-md border bg-card p-3 transition-colors
                  {selected[app.id] ? 'border-accent/60 bg-card-hi' : 'border-line hover:bg-card-hi'}"
              >
                <input type="checkbox" class="mt-0.5 accent-[#8fb5a0]" bind:checked={selected[app.id]} />
                <span class="min-w-0">
                  <span class="block truncate text-[13px] font-medium">{app.title}</span>
                  <span class="block truncate font-mono text-[10.5px] text-faint">{app.packages[0]?.version} · {app.id}</span>
                  {#if app.note}
                    <span class="mt-1 flex gap-1 text-[11.5px] text-moderate/90"><TriangleAlert size={12} class="mt-0.5 shrink-0" />{app.note}</span>
                  {/if}
                </span>
              </label>
            {/each}
          </div>
        </section>
      {/if}
    {/each}

    <div class="sticky bottom-4 mt-6 flex items-center gap-3 rounded-lg border border-[#3a3935] bg-panel p-2.5 shadow-lg shadow-black/50">
      <label class="flex items-center gap-2 text-[12.5px] {elevated ? 'text-mute' : 'text-faint'}" title={elevated ? "" : "Requiere administrador"}>
        <input
          type="checkbox"
          class="accent-[#8fb5a0]"
          checked={deprovision && elevated}
          onchange={(e) => (deprovision = e.currentTarget.checked)}
          disabled={!elevated}
        />
        Evitar que se reinstalen en cuentas nuevas
      </label>
      <span class="flex-1"></span>
      {#if confirming}
        <span class="text-[12.5px] text-moderate">¿Quitar {chosen.length} apps?</span>
        <button class="rounded-lg px-3 py-1.5 text-[13px] text-mute hover:bg-card-hi" onclick={() => (confirming = false)}>Cancelar</button>
        <button class="inline-flex items-center gap-2 rounded-md bg-advanced px-3 py-1 text-[13px] font-medium text-bg disabled:opacity-50" disabled={working} onclick={remove}>
          {#if working}<LoaderCircle size={14} class="animate-spin" />{/if} Sí, quitar
        </button>
      {:else}
        <button
          class="rounded-md border border-line px-3 py-1 text-[13px] disabled:opacity-40"
          disabled={!chosen.length}
          onclick={() => (confirming = true)}
        >
          Quitar {chosen.length || ""} seleccionadas
        </button>
      {/if}
    </div>
  {/if}
</div>
