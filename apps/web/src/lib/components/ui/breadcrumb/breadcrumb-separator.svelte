<script lang="ts">
	import { ChevronRight } from "lucide-svelte";
	import { cn, type WithElementRef } from "$lib/utils";
	import type { HTMLAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLLIElement>> = $props();
</script>

<li
	bind:this={ref}
	role="presentation"
	aria-hidden="true"
	class={cn("select-none text-[var(--text-muted)] [&>svg]:h-3 [&>svg]:w-3", className)}
	{...restProps}
>
	{#if children}
		{@render children()}
	{:else}
		<ChevronRight />
	{/if}
</li>
