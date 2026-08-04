<!-- Responsibility: stable GraphViewer entry component hosting Svelte inputs/outputs and the View Runtime DOM. -->
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { RuntimeStateEventDetail } from '../runtime-loading';
  import type { UsageBlock } from '../billing/entitlement-gate';
  import type { PathSeg } from '../store/tree-path';
  import type { ColumnNavigatorState } from './graph-viewer/column-navigator/types';
  import type { SharedWorkspaceMutationTarget } from '../share/share-workspace-lifecycle';
  import type { SupportedEditorLanguageId } from '../monaco/language-support';
  import GraphViewRuntime from './GraphViewRuntime.svelte';

  type GraphSearchTarget = 'node' | 'key' | 'value';

  type GraphSearchResult = {
    nodeId: number | undefined;
    target: GraphSearchTarget;
    label: string;
    path: PathSeg[];
    pathText: string;
  };

  export let active = true;
  export let synchronizedRuntimeLoading = false;
  export let readonly = false;
  export let onFileDrop: (event: DragEvent) => void | Promise<void> = () => {};
  export let onRequestImportFile: (payload: { sourceFormat: string; targetFormat: string; accept: string[] }) => void | Promise<void> = () => {};
  export let onLoadExample: (example: string, language: SupportedEditorLanguageId) => void | Promise<void> = () => {};
  export let onEntitlementBlocked: (block: UsageBlock) => void = () => {};
  export let ensureSharedWorkspacePromoted: (target: SharedWorkspaceMutationTarget) => Promise<boolean> = async () => true;

  const dispatch = createEventDispatcher<{
    navigation: unknown;
    'runtime-state': RuntimeStateEventDetail;
    'column-navigator-state': ColumnNavigatorState;
  }>();
  let runtime: GraphViewRuntime | null = null;

  export function previewSearchResult(result: GraphSearchResult): void {
    runtime?.previewSearchResult(result);
  }

  export function commitSearchPreview(): void {
    runtime?.commitSearchPreview();
  }

  export async function cancelSearchPreview(): Promise<void> {
    await runtime?.cancelSearchPreview();
  }

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

  export function isGraphInteractive(): boolean {
    return runtime?.isGraphInteractive() ?? false;
  }

  export function getColumnNavigatorActivePath(): PathSeg[] {
    return runtime?.getColumnNavigatorActivePath() ?? [];
  }

  export async function restoreColumnNavigatorPath(path: PathSeg[]): Promise<boolean> {
    return await runtime?.restoreColumnNavigatorPath(path) ?? false;
  }

  export function collapseColumnNavigator(): void {
    runtime?.collapseColumnNavigator();
  }

  export function expandColumnNavigator(): void {
    runtime?.expandColumnNavigator();
  }

  export function pinColumnNavigatorCollapsed(): void {
    runtime?.pinColumnNavigatorCollapsed();
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

  export async function applyColumnNavigatorNavigationPath(path: PathSeg[]): Promise<void> {
    await runtime?.applyColumnNavigatorNavigationPath(path);
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
  {active}
  {synchronizedRuntimeLoading}
  {readonly}
  {onFileDrop}
  {onRequestImportFile}
  {onLoadExample}
  {onEntitlementBlocked}
  {ensureSharedWorkspacePromoted}
  on:navigation={(event) => dispatch('navigation', event.detail)}
  on:runtime-state={(event) => dispatch('runtime-state', event.detail)}
  on:column-navigator-state={(event) => dispatch('column-navigator-state', event.detail)}
/>
