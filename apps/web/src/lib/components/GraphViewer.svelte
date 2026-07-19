<!-- 职责：GraphViewer 稳定入口组件：Svelte 输入/输出与 View Runtime DOM 宿主 -->
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { RuntimeStateEventDetail } from '../runtime-loading';
  import type { UsageBlock } from '../billing/entitlement-gate';
  import type { PathSeg } from '../store/tree-path';
  import GraphViewRuntime from './GraphViewRuntime.svelte';

  type GraphSearchTarget = 'node' | 'key' | 'value';

  type GraphSearchResult = {
    nodeId: number | undefined;
    target: GraphSearchTarget;
    label: string;
    path: PathSeg[];
    pathText: string;
  };

  export let enableRevealSync = true;
  export let synchronizedRuntimeLoading = false;
  export let readonly = false;

  const dispatch = createEventDispatcher<{ reveal: unknown; 'runtime-state': RuntimeStateEventDetail }>();
  let runtime: GraphViewRuntime | null = null;

  export function revealSearchResult(result: GraphSearchResult): void {
    runtime?.revealSearchResult(result);
  }

  export function revealPath(
    path: PathSeg[],
    options: { target: 'key' | 'value' | 'node' | undefined; navigate: boolean | undefined },
  ): Promise<boolean> {
    return runtime?.revealPath(path, options) ?? Promise.resolve(false);
  }

  export async function waitForGraphReady(): Promise<boolean> {
    return await runtime?.waitForGraphReady() ?? false;
  }

  export function getSubgraphWorkspacePaths(): PathSeg[][] {
    return runtime?.getSubgraphWorkspacePaths() ?? [];
  }

  export async function restoreSubgraphWorkspacePaths(paths: PathSeg[][]): Promise<boolean> {
    return await runtime?.restoreSubgraphWorkspacePaths(paths) ?? false;
  }

  export async function exportImage(): Promise<void> {
    await runtime?.exportImage();
  }

  export function zoomIn(): void {
    runtime?.zoomIn();
  }

  export function zoomOut(): void {
    runtime?.zoomOut();
  }

  export function showEntitlementOverlay(block: UsageBlock): void {
    runtime?.showEntitlementOverlay(block);
  }
</script>

<GraphViewRuntime
  bind:this={runtime}
  {enableRevealSync}
  {synchronizedRuntimeLoading}
  {readonly}
  on:reveal={(event) => dispatch('reveal', event.detail)}
  on:runtime-state={(event) => dispatch('runtime-state', event.detail)}
/>
