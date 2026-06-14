<script lang="ts">
	import { cn, type WithElementRef } from "$lib/utils";
	import type { HTMLAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		class: className,
		orientation = "vertical",
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLDivElement>> & {
		orientation?: "horizontal" | "vertical";
	} = $props();
</script>

<div
	bind:this={ref}
	role="separator"
	data-slot="button-group-separator"
	data-orientation={orientation}
	class={cn(
		"bg-[var(--border-muted)] data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-px",
		className
	)}
	{...restProps}
></div>
