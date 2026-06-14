<script lang="ts">
	import { cn, type WithElementRef } from "$lib/utils";
	import type { HTMLAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		class: className,
		orientation = "horizontal",
		variant = "default",
		children,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLDivElement>> & {
		orientation?: "horizontal" | "vertical";
		variant?: "default" | "segmented-outline";
	} = $props();
</script>

<div
	bind:this={ref}
	role="group"
	data-slot="button-group"
	data-orientation={orientation}
	data-variant={variant}
	class={cn(
		"inline-flex items-center gap-1 data-[orientation=vertical]:flex-col",
		variant === "segmented-outline" &&
			"gap-0 rounded-[8px] border border-[var(--border-muted)] bg-[var(--panel-bg)] shadow-none data-[orientation=vertical]:items-stretch [&>[data-slot=button]]:rounded-none [&>[data-slot=button]]:border-0 [&>[data-slot=button][data-variant=ghost]]:bg-transparent [&>[data-slot=button][data-variant=ghost]]:hover:bg-[var(--panel-bg-alt)] [&>[data-slot=button]:not(:first-child)]:border-l [&>[data-slot=button]:not(:first-child)]:border-[var(--border-muted)] [&>[data-button-group-item]]:inline-flex [&>[data-button-group-item]]:items-center [&>[data-button-group-item]>[data-slot=button]]:rounded-none [&>[data-button-group-item]>[data-slot=button]]:border-0 [&>[data-button-group-item]>[data-slot=button][data-variant=ghost]]:bg-transparent [&>[data-button-group-item]>[data-slot=button][data-variant=ghost]]:hover:bg-[var(--panel-bg-alt)] [&>[data-button-group-item]:not(:first-child)]:border-l [&>[data-button-group-item]:not(:first-child)]:border-[var(--border-muted)] data-[orientation=vertical]:[&>[data-slot=button]:not(:first-child)]:border-l-0 data-[orientation=vertical]:[&>[data-slot=button]:not(:first-child)]:border-t data-[orientation=vertical]:[&>[data-button-group-item]:not(:first-child)]:border-l-0 data-[orientation=vertical]:[&>[data-button-group-item]:not(:first-child)]:border-t",
		className
	)}
	{...restProps}
>
	{@render children?.()}
</div>
