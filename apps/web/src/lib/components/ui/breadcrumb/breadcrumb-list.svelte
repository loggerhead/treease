<script lang="ts">
	import { cn, type WithElementRef } from "$lib/utils";
	import type { HTMLOlAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: WithElementRef<HTMLOlAttributes> = $props();
</script>

<ol
	bind:this={ref}
	class={cn("flex flex-wrap items-center gap-1.5 text-[var(--text-muted)] sm:gap-2.5", className)}
	{...restProps}
>
	{@render children?.()}
</ol>
