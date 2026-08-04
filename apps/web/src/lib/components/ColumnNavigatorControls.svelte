<script lang="ts">
  import { ChevronDown, ChevronLeft, ChevronRight, ChevronUp, X } from 'lucide-svelte';
  import type { ColumnNavigatorState } from './graph-viewer/column-navigator/types';

  export let state: ColumnNavigatorState | null = null;
  export let onBack: () => void | Promise<void> = () => {};
  export let onForward: () => void | Promise<void> = () => {};
  export let onCollapse: () => void = () => {};
  export let onPinCollapsed: () => void = () => {};
  export let onExpand: () => void = () => {};
</script>

{#if state}
  <div class="column-navigator-controls" data-testid="column-navigator-controls">
    <button type="button" aria-label="Back in workspace history" data-testid="column-navigator-back" disabled={!state.canGoBack} onclick={() => void onBack()}><ChevronLeft size={13} /></button>
    <button type="button" aria-label="Forward in workspace history" data-testid="column-navigator-forward" disabled={!state.canGoForward} onclick={() => void onForward()}><ChevronRight size={13} /></button>
    <button type="button" aria-label={state.collapsed ? 'Expand column navigator' : 'Collapse column navigator'} data-testid={state.collapsed ? 'column-navigator-expand' : 'column-navigator-collapse'} onclick={state.collapsed ? onExpand : onCollapse}>{#if state.collapsed}<ChevronUp size={14} />{:else}<ChevronDown size={14} />{/if}</button>
    <button type="button" aria-label="Keep navigator collapsed" data-testid="column-navigator-pin-collapsed" disabled={state.collapsed} onclick={onPinCollapsed}><X size={14} /></button>
  </div>
{/if}

<style>
  .column-navigator-controls { display: inline-flex; align-items: center; gap: 2px; margin-right: 6px; }
  button { display: inline-flex; align-items: center; justify-content: center; border: 0; border-radius: 4px; color: var(--text-muted); background: transparent; }
  button:hover:not(:disabled) { color: var(--text-primary); background: var(--panel-bg-alt); }
  button:disabled { opacity: .4; }
</style>
