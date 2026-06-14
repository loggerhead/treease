<script lang="ts">
	import { cn, type WithElementRef } from "$lib/utils";
	import type { HTMLAnchorAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: WithElementRef<HTMLAnchorAttributes> = $props();
</script>

<a
	bind:this={ref}
	class={cn("text-[12px] transition-colors hover:text-[var(--text-primary)]", className)}
	{...restProps}
>
	{@render children?.()}
</a>
