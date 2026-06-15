<script lang="ts">
  import { onMount } from 'svelte';
  import ViewportPanel from '$lib/components/ViewportPanel.svelte';
  import { settingsStore } from '$lib/settings/settings-store';
  import { getSharedWasmWorkerClient } from '$lib/wasm/wasm-worker-singleton';
  import { applyCliGraphResultToEditorStore } from '$lib/cli-graph/apply-result';
  import {
    fetchCliGraphResult,
    readCliGraphTokenFromSearch,
    type CliGraphResult,
  } from '$lib/cli-graph/result-client';
  import wasmUrl from '@core-wasm/pkg/core.wasm?url';

  let result: CliGraphResult | null = null;
  let errorMessage = '';
  let loading = true;

  async function loadCliGraphResult(): Promise<void> {
    loading = true;
    errorMessage = '';
    try {
      const token = readCliGraphTokenFromSearch(window.location.search);
      const loaded = await fetchCliGraphResult(token);
      applyCliGraphResultToEditorStore(token, loaded);
      result = loaded;
      document.title = `Treease CLI graph - ${loaded.sourceLabel || loaded.expression || 'result'}`;
    } catch (error) {
      result = null;
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    settingsStore.load();
    void getSharedWasmWorkerClient().catch(() => {});
    void loadCliGraphResult();
  });
</script>

<svelte:head>
  <link rel="preload" as="fetch" href={wasmUrl} crossorigin="anonymous" />
</svelte:head>

<main class="h-screen w-screen overflow-hidden bg-[var(--app-bg)] text-[var(--text-primary)]">
  {#if loading}
    <section
      class="grid h-full w-full place-items-center bg-[var(--panel-bg)] text-[13px] text-[var(--text-muted)]"
      data-testid="cli-graph-loading"
    >
      Loading CLI graph...
    </section>
  {:else if errorMessage}
    <section class="grid h-full w-full place-items-center bg-[var(--panel-bg)] px-6" data-testid="cli-graph-error">
      <div class="max-w-xl rounded-[18px] border border-[rgba(203,42,47,0.22)] bg-white p-5 shadow-[0_18px_60px_rgba(15,23,42,0.12)]">
        <p class="text-[12px] font-semibold uppercase tracking-[0.18em] text-[rgb(203,42,47)]">CLI graph unavailable</p>
        <p class="mt-3 text-[14px] leading-6 text-[var(--text-primary)]">{errorMessage}</p>
      </div>
    </section>
  {:else if result}
    <ViewportPanel
      viewMode="graph"
      graphOnly={true}
      readonlyGraph={true}
      enableRevealSync={false}
      synchronizedRuntimeLoading={false}
    />
  {/if}
</main>
