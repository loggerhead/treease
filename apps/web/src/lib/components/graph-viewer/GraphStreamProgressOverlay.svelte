<script lang="ts">
  import { cubicOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';
  import { Progress } from '$lib/components/ui/progress';
  import type { GraphStreamProgressState } from './graph-stream-progress';

  export let state: GraphStreamProgressState;

  $: progressText =
    state.phase === 'flushing' || state.phase === 'finishing' ? '...' : `${Math.round(state.value)}%`;

  const motion = {
    y: 6,
    duration: 150,
    opacity: 0.08,
  };
</script>

{#if state.visible}
  <div
    class="pointer-events-none absolute bottom-4 right-4 z-10 w-72 rounded-[12px] border border-[#e2e8f0] bg-white/95 p-3 shadow-[0_12px_28px_rgba(15,23,42,0.14)] backdrop-blur"
    transition:fly={{ ...motion, easing: cubicOut }}
  >
    <div class="mb-2 flex items-center justify-between gap-3 text-[12px] leading-none">
      <span class="font-medium text-[#0f172a]">{state.label}</span>
      <span class="font-mono text-[#64748b]">{progressText}</span>
    </div>
    <Progress value={state.value} max={100} class="h-2 w-full" />
    {#if state.detail}
      <div class="mt-2 text-[11px] text-[#64748b]">{state.detail}</div>
    {/if}
  </div>
{/if}
