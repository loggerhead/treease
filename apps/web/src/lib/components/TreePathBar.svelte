<script lang="ts">
  import { Check, Copy } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';
  import * as Breadcrumb from './ui/breadcrumb';
  import { buildReadablePath, isPathSegIndex, pathSegKeyValue, type PathSeg } from '../store/tree-path';

  export let value: PathSeg[] = [];
  export let disabled = false;

  type TreePathBarCrumb = { label: string; value: string; segments: PathSeg[] };

  let copied = false;
  const dispatch = createEventDispatcher();
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  function handleTreePathClick(segments: PathSeg[]) {
    if (disabled) return;
    dispatch('select', segments);
  }

  function handleCrumbClick(event: MouseEvent, segments: PathSeg[]) {
    event.preventDefault();
    handleTreePathClick(segments);
  }

  function handleCopyClick() {
    if (disabled) return;
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
    const crumbs: TreePathBarCrumb[] = [{ label: '$', value: '$', segments: [] }];
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
  let crumbs: TreePathBarCrumb[] = [];
  let displayPath = '$';

  $: normalizedValue = Array.isArray(value) ? value : [];
  $: crumbs = buildCrumbs(normalizedValue);
  $: displayPath = buildReadablePath(normalizedValue);
</script>

<Breadcrumb.Root class={`group h-full flex items-center ${disabled ? 'tree-path-bar--disabled' : ''}`} data-testid="tree-path-bar" aria-disabled={disabled}>
  <Breadcrumb.List class="flex-nowrap whitespace-nowrap overflow-x-auto h-full items-center">
    {#each crumbs as crumb, index (index)}
      {@const typedCrumb = crumb as TreePathBarCrumb}
      <Breadcrumb.Item class="inline-block cursor-pointer max-w-[var(--max-key-length)] truncate">
        <Breadcrumb.Link
          href="#"
          class={`block truncate ${disabled ? 'tree-path-bar__link--disabled' : ''}`}
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
    <Breadcrumb.Item>
      <button
        class="ml-1 inline-flex items-center opacity-0 transition-opacity duration-2000 ease-in-out group-hover:opacity-100"
        title="Copy tree path"
        disabled={disabled}
        onclick={handleCopyClick}
      >
        {#if copied}<Check size={12} class="text-(--text-primary)" />{:else}<Copy size={12} class="text-(--text-muted)" />{/if}
      </button>
    </Breadcrumb.Item>
  </Breadcrumb.List>
</Breadcrumb.Root>

<style>
  button { display: inline-flex; width: 22px; height: 22px; align-items: center; justify-content: center; border: 0; border-radius: 4px; color: var(--text-muted); background: transparent; transition: var(--control-transition); }
  button:hover:not(:disabled) { color: var(--text-primary); background: var(--panel-bg-alt); }
  button:disabled { opacity: .4; }
  button:focus-visible { outline: none; box-shadow: var(--focus-ring); }
  :global(.tree-path-bar--disabled) { opacity: .48; }
  :global(.tree-path-bar__link--disabled) { cursor: not-allowed; pointer-events: none; }
</style>
