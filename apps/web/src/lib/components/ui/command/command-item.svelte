<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cn } from "$lib/utils";

	let {
		class: className,
		children,
		...restProps
	}: CommandPrimitive.ItemProps = $props();
</script>

<CommandPrimitive.Item
	class={cn(
		"relative flex w-full cursor-default items-center gap-2 rounded-[8px] px-2.5 py-1.5 text-left text-[13px] outline-none select-none transition-colors duration-150 data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[highlighted]:bg-[var(--item-hover-bg)] data-[highlighted]:text-[var(--text-primary)] data-[selected]:bg-[var(--item-active-bg)] data-[selected]:text-[var(--text-primary)]",
		className
	)}
	{...restProps}
>
	{@render children?.()}
</CommandPrimitive.Item>
