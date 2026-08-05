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

  export let active = true;
  export let sidecarTabId = '';
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
    'column-navigator-state': { tabId: string; state: ColumnNavigatorState };
    'graph-viewport-state': { tabId: string; viewport: { x: number; y: number; scaleX: number; scaleY: number } | null };
  }>();
  let runtime: GraphViewRuntime | null = null;

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

  export async function restoreColumnNavigatorNavigationState(state: {
    activePath: PathSeg[];
    canGoBack: boolean;
    canGoForward: boolean;
    collapsed: boolean;
  }): Promise<void> {
    await runtime?.restoreColumnNavigatorNavigationState(state);
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

  export async function applyColumnNavigatorNavigationProjection(state: {
    activePath: PathSeg[];
    canGoBack: boolean;
    canGoForward: boolean;
    materializeColumns: boolean;
    expanded: boolean;
  }): Promise<void> {
    await runtime?.applyColumnNavigatorNavigationProjection(state);
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

  export function restoreGraphViewport(state: { x: number; y: number; scaleX: number; scaleY: number } | null): void {
    runtime?.restoreGraphViewport(state);
  }

  export function getGraphViewport(): { x: number; y: number; scaleX: number; scaleY: number } | null {
    return runtime?.getGraphViewport() ?? null;
  }

  export function cancelGraphViewportTransition(): void {
    runtime?.cancelGraphViewportTransition();
  }

  export function waitForGraphViewportTransition(): Promise<void> {
    return runtime?.waitForGraphViewportTransition() ?? Promise.resolve();
  }
</script>

<GraphViewRuntime
  bind:this={runtime}
  {sidecarTabId}
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
  on:column-navigator-state={(event) => { if (sidecarTabId) dispatch('column-navigator-state', { tabId: sidecarTabId, state: event.detail }); }}
  on:graph-viewport-state={(event) => { if (sidecarTabId) dispatch('graph-viewport-state', { tabId: sidecarTabId, viewport: event.detail }); }}
/>
