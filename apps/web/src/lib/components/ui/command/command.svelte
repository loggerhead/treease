<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cn } from "$lib/utils";

	let {
		class: className,
		children,
		...restProps
	}: CommandPrimitive.RootProps = $props();
</script>

<CommandPrimitive.Root
	class={cn(
		"flex h-full w-full flex-col overflow-hidden rounded-[14px] bg-white text-[var(--text-primary)]",
		className
	)}
	{...restProps}
>
	{@render children?.()}
</CommandPrimitive.Root>
