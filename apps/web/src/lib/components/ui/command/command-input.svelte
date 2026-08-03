<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cn } from "$lib/utils";

	let {
		class: className,
		children,
		ref = $bindable(null),
		...restProps
	}: CommandPrimitive.InputProps = $props();

	export function focus(): void {
		ref?.focus();
	}
</script>

<CommandPrimitive.Input
	bind:ref
	class={cn(
		"flex h-10 w-full rounded-md bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50",
		className
	)}
	{...restProps}
>
	{@render children?.()}
</CommandPrimitive.Input>
