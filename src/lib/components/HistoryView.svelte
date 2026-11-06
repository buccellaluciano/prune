<script lang="ts">
  import { History, LoaderCircle, Undo2 } from "@lucide/svelte";
  import { api, type JournalEntry } from "$lib/api";
  import { store } from "$lib/store.svelte";
  import { onMount } from "svelte";

  let entries = $state<JournalEntry[] | null>(null);
  let working = $state<number | null>(null);
  /** Entrada con valores que cambiaron después: se pide confirmación antes de pisarlos. */
  let conflict = $state<{ id: number; items: string[] } | null>(null);

  async function load() {
    entries = await api.history().catch((e) => {
      store.toast("error", String(e));
      return [];
    });
  }
  onMount(load);

  async function undo(e: JournalEntry, force = false) {
    working = e.id;
    try {
      const r = await api.undo(e.id, force);
      if (r.conflicts.length) {
        conflict = { id: e.id, items: r.conflicts };
        if (r.warning) store.toast("error", r.warning);
        return;
      }
      conflict = null;
      store.report(r, "deshechos");
      await Promise.all([load(), store.load()]);
    } catch (err) {
      store.toast("error", String(err));
    } finally {
      working = null;
    }
  }

  const fmt = new Intl.DateTimeFormat("es-AR", { dateStyle: "medium", timeStyle: "short" });
</script>

<div class="mx-auto max-w-3xl">
  <h1 class="font-display text-[20px] font-semibold">Historial</h1>
  <p class="mt-1 mb-5 text-[13px] text-mute">Cada cambio guarda el valor anterior exacto del registro. Deshacer lo restaura tal cual estaba.</p>

  {#if !entries}
    <div class="flex items-center gap-2 text-[13px] text-mute"><LoaderCircle size={16} class="animate-spin" /> Cargando…</div>
  {:else if entries.length === 0}
    <div class="rounded-lg border border-line bg-card p-10 text-center text-[13px] text-mute">
      <History size={26} class="mx-auto mb-3 text-faint" /> Todavía no aplicaste cambios.
    </div>
  {:else}
    <ol class="relative space-y-2 border-l border-line pl-5">
      {#each entries as e (e.id)}
        <li class="relative rounded-md border border-line bg-card px-3 py-2.5 {e.undone ? 'opacity-55' : ''}">
          <span class="absolute top-4 -left-[24.5px] size-2 rounded-full {e.undone ? 'bg-line' : e.removed_apps.length ? 'bg-advanced' : 'bg-accent'}"></span>
          <div class="flex items-center gap-3">
            <div class="min-w-0 flex-1">
              <div class="truncate text-[13px] font-medium">{e.label}</div>
              <div class="text-[11.5px] text-mute">
                {fmt.format(new Date(e.at * 1000))} ·
                {#if e.removed_apps.length}{e.removed_apps.length} apps quitadas{:else}{e.changes.length} valores{/if}
                {#if e.undone} · deshecho{/if}
              </div>
            </div>
            {#if !e.undone && e.changes.length && conflict?.id !== e.id}
              <button
                class="inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[12.5px] text-mute hover:bg-card-hi hover:text-ink disabled:opacity-50"
                disabled={working !== null}
                onclick={() => undo(e)}
              >
                {#if working === e.id}<LoaderCircle size={13} class="animate-spin" />{:else}<Undo2 size={13} />{/if} Deshacer
              </button>
            {/if}
          </div>
          {#if conflict?.id === e.id}
            <div class="mt-2.5 border-t border-line pt-2.5 text-[12.5px]">
              <p class="text-moderate">Estos valores cambiaron después de este cambio. Deshacer los va a pisar:</p>
              <ul class="mt-1 space-y-0.5 font-mono text-[11px] text-mute">
                {#each conflict.items as c}<li>{c}</li>{/each}
              </ul>
              <div class="mt-2 flex justify-end gap-2">
                <button class="rounded-md px-2.5 py-1 text-[12px] text-mute hover:text-ink" onclick={() => (conflict = null)}>Cancelar</button>
                <button
                  class="rounded-md border border-moderate/50 px-2.5 py-1 text-[12px] text-moderate hover:bg-moderate/10 disabled:opacity-50"
                  disabled={working !== null}
                  onclick={() => undo(e, true)}
                >
                  Deshacer igual
                </button>
              </div>
            </div>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</div>
