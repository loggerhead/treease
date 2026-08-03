<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cn } from "$lib/utils";

	let {
		class: className,
		children,
		...restProps
	}: CommandPrimitive.GroupProps = $props();
</script>

<CommandPrimitive.Group
	class={cn("overflow-hidden p-1 text-foreground", className)}
	{...restProps}
>
	{@render children?.()}
</CommandPrimitive.Group>
