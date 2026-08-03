<script lang="ts">
  import { PanelLeftOpen, PanelRightOpen } from 'lucide-svelte';
  import Tooltip from '../../Tooltip.svelte';
  import * as ButtonGroup from '../button-group';
  import { IconButton } from '../button';
  import type { SplitLayoutMode } from './split-layout-controller';

  export let mode: SplitLayoutMode;
  export let leftPx: number;
  export let expandLeftLabel: string;
  export let expandRightLabel: string;
  export let testId = '';
  export let onExpand: () => void;
</script>

{#if mode === 'left-only'}
  <div class="splitter-control splitter-control--collapsed" style:left={`${leftPx}px`}>
    <ButtonGroup.Root orientation="vertical" variant="segmented-outline" class="shadow-none">
      <Tooltip content={expandRightLabel} side="left">
        <IconButton class="text-[var(--text-primary)]" aria-label={expandRightLabel} data-testid={testId} on:click={onExpand}>
          <PanelRightOpen size={12} />
        </IconButton>
      </Tooltip>
    </ButtonGroup.Root>
  </div>
{:else if mode === 'right-only'}
  <div class="splitter-control splitter-control--collapsed" style:left={`${leftPx}px`}>
    <ButtonGroup.Root orientation="vertical" variant="segmented-outline" class="shadow-none">
      <Tooltip content={expandLeftLabel} side="right">
        <IconButton class="text-[var(--text-primary)]" aria-label={expandLeftLabel} data-testid={testId} on:click={onExpand}>
          <PanelLeftOpen size={12} />
        </IconButton>
      </Tooltip>
    </ButtonGroup.Root>
  </div>
{/if}
