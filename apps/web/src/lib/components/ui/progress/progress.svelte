<script lang="ts">
  import { cn, type WithElementRef } from '$lib/utils';
  import type { HTMLAttributes } from 'svelte/elements';

  let {
    ref = $bindable(null),
    class: className,
    value = 0,
    max = 100,
    "data-slot": dataSlot = 'progress',
    ...restProps
  }: WithElementRef<HTMLAttributes<HTMLDivElement>> & {
    value?: number;
    max?: number;
    'data-slot'?: string;
  } = $props();

  function clampValue(nextValue: number, nextMax: number): number {
    if (!Number.isFinite(nextValue) || !Number.isFinite(nextMax) || nextMax <= 0) {
      return 0;
    }
    return Math.max(0, Math.min(nextValue, nextMax));
  }

  const boundedValue = $derived(clampValue(Number(value), Number(max)));
  const ratio = $derived(max > 0 ? boundedValue / max : 0);
  const translatePercent = $derived(Math.round((1 - ratio) * 10000) / 100);
</script>

<div
  bind:this={ref}
  data-slot={dataSlot}
  role="progressbar"
  aria-valuemin={0}
  aria-valuemax={max}
  aria-valuenow={boundedValue}
  class={cn('bg-primary/15 relative h-2 w-full overflow-hidden rounded-full', className)}
  {...restProps}
>
  <div
    data-slot="progress-indicator"
    class="bg-primary h-full w-full flex-1 transition-transform duration-150 ease-out"
    style={`transform: translateX(-${translatePercent}%);`}
  ></div>
</div>
