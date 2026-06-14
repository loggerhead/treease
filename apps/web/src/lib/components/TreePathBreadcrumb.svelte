<script lang="ts">
  import { Check, Copy } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';
  import * as Breadcrumb from './ui/breadcrumb';
  import { buildReadablePath, isPathSegIndex, pathSegKeyValue, type PathSeg } from '../store/tree-path';

  export let value: PathSeg[] = [];

  type TreePathCrumb = { label: string; value: string; segments: PathSeg[] };

  let copied = false;
  const dispatch = createEventDispatcher();
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  function handleTreePathClick(segments: PathSeg[]) {
    dispatch('select', segments);
  }

  function handleCrumbClick(event: MouseEvent, segments: PathSeg[]) {
    event.preventDefault();
    handleTreePathClick(segments);
  }

  function handleCopyClick() {
    if (!navigator?.clipboard) return;
    navigator.clipboard.writeText(displayPath).then(() => {
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => {
        copied = false;
      }, 1000);
    });
  }

  function segmentLabel(segment: PathSeg) {
    if (isPathSegIndex(segment)) return `[${segment.index}]`;
    return pathSegKeyValue(segment);
  }

  function buildJoinedPathValue(segments: PathSeg[]) {
    if (!Array.isArray(segments) || segments.length === 0) return '$';
    return segments.map((segment) => (isPathSegIndex(segment) ? String(segment.index) : pathSegKeyValue(segment))).join('.');
  }

  function buildCrumbs(segments: PathSeg[]) {
    const crumbs: TreePathCrumb[] = [{ label: '$', value: '$', segments: [] }];
    const currentSegments: PathSeg[] = [];
    for (const segment of segments) {
      currentSegments.push(segment);
      crumbs.push({
        label: segmentLabel(segment),
        value: buildJoinedPathValue(currentSegments),
        segments: [...currentSegments],
      });
    }
    return crumbs;
  }

  let normalizedValue: PathSeg[] = [];
  let crumbs: TreePathCrumb[] = [];
  let displayPath = '$';

  $: normalizedValue = Array.isArray(value) ? value : [];
  $: crumbs = buildCrumbs(normalizedValue);
  $: displayPath = buildReadablePath(normalizedValue);
</script>

<Breadcrumb.Root class="group h-full flex items-center">
  <Breadcrumb.List class="flex-nowrap whitespace-nowrap overflow-x-auto h-full items-center">
    {#each crumbs as crumb, index (index)}
      {@const typedCrumb = crumb as TreePathCrumb}
      <Breadcrumb.Item class="inline-block cursor-pointer max-w-[var(--max-key-length)] truncate">
        <Breadcrumb.Link
          href="#"
          class="block truncate"
          onclick={(event: MouseEvent) => handleCrumbClick(event, typedCrumb.segments)}
          aria-label={`Tree path ${typedCrumb.value}`}
          data-testid={`tree-path-crumb-${index}`}
          title={typedCrumb.value}
        >
          {typedCrumb.label}
        </Breadcrumb.Link>
      </Breadcrumb.Item>
      {#if index < crumbs.length - 1}
        <Breadcrumb.Separator />
      {/if}
    {/each}
    <button
      class="ml-1 inline-flex items-center opacity-0 transition-opacity duration-2000 ease-in-out group-hover:opacity-100"
      title="Copy tree path"
      on:click={handleCopyClick}
    >
      {#if copied}
        <Check size={12} class="text-(--text-primary)" />
      {:else}
        <Copy size={12} class="text-(--text-muted)" />
      {/if}
    </button>
  </Breadcrumb.List>
</Breadcrumb.Root>
