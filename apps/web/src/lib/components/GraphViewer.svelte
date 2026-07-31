<!-- Responsibility: stable GraphViewer entry component hosting Svelte inputs/outputs and the View Runtime DOM. -->
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { RuntimeStateEventDetail } from '../runtime-loading';
  import type { UsageBlock } from '../billing/entitlement-gate';
  import type { PathSeg } from '../store/tree-path';
  import type { ColumnNavigatorState } from './graph-viewer/column-navigator/types';
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
  export let onFileDrop: (event: DragEvent) => void | Promise<void> = () => {};
  export let onEntitlementBlocked: (block: UsageBlock) => void = () => {};

  const dispatch = createEventDispatcher<{
    reveal: unknown;
    'runtime-state': RuntimeStateEventDetail;
    'column-navigator-state': ColumnNavigatorState;
  }>();
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

  export function getColumnNavigatorActivePath(): PathSeg[] {
    return runtime?.getColumnNavigatorActivePath() ?? [];
  }

  export async function restoreColumnNavigatorPath(path: PathSeg[]): Promise<boolean> {
    return await runtime?.restoreColumnNavigatorPath(path) ?? false;
  }

  export async function goColumnNavigatorBack(): Promise<void> {
    await runtime?.goColumnNavigatorBack();
  }

  export async function goColumnNavigatorForward(): Promise<void> {
    await runtime?.goColumnNavigatorForward();
  }

  export async function selectColumnNavigatorPath(path: PathSeg[]): Promise<void> {
    await runtime?.selectColumnNavigatorPath(path);
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
  {onFileDrop}
  {onEntitlementBlocked}
  on:reveal={(event) => dispatch('reveal', event.detail)}
  on:runtime-state={(event) => dispatch('runtime-state', event.detail)}
  on:column-navigator-state={(event) => dispatch('column-navigator-state', event.detail)}
/>
