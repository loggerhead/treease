<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cn } from "$lib/utils";

	let {
		class: className,
		children,
		...restProps
	}: CommandPrimitive.EmptyProps = $props();
</script>

<CommandPrimitive.Empty
	class={cn("py-6 text-center text-sm", className)}
	{...restProps}
>
	{@render children?.()}
</CommandPrimitive.Empty>
