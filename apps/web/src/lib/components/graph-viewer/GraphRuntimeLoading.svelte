<script lang="ts">
  import RuntimeLoadingBar from '../runtime-loading/RuntimeLoadingBar.svelte';
  import {
    GRAPH_RUNTIME_LOADING_EDGES,
    GRAPH_RUNTIME_LOADING_NODES,
    GRAPH_RUNTIME_LOADING_NODE_BARS,
    GRAPH_RUNTIME_LOADING_VIEWBOX_PADDING,
    getGraphRuntimeLoadingViewBox,
    type GraphRuntimeLoadingEdge,
    type GraphRuntimeLoadingNodeBar,
    type GraphRuntimeLoadingNode,
  } from './graph-runtime-loading-data';

  export let nodes: GraphRuntimeLoadingNode[] = GRAPH_RUNTIME_LOADING_NODES;
  export let edges: GraphRuntimeLoadingEdge[] = GRAPH_RUNTIME_LOADING_EDGES;
  export let nodeBars: GraphRuntimeLoadingNodeBar[][] = GRAPH_RUNTIME_LOADING_NODE_BARS;

  $: viewBox = getGraphRuntimeLoadingViewBox(nodes, edges, GRAPH_RUNTIME_LOADING_VIEWBOX_PADDING);

  function edgePath(edge: GraphRuntimeLoadingEdge): string {
    return `M ${edge.fromX} ${edge.fromY} C ${edge.c1x} ${edge.c1y} ${edge.c2x} ${edge.c2y} ${edge.toX} ${edge.toY}`;
  }
</script>

<div class="graph-runtime-loading" role="status" aria-live="polite" aria-label="Graph loading status">

  <svg
    class="graph-runtime-loading__scene"
    viewBox={viewBox}
    preserveAspectRatio="xMidYMid meet"
    aria-hidden="true"
  >
    <g class="graph-runtime-loading__edges">
      {#each edges as edge}
        <path class="graph-runtime-loading__edge" d={edgePath(edge)}></path>
      {/each}
    </g>

    <g class="graph-runtime-loading__nodes">
      {#each nodes as node, index}
        {@const bars = nodeBars[index] ?? []}
        <g class="graph-runtime-loading__node">
          <rect class="graph-runtime-loading__node-shell" x={node.x} y={node.y} width={node.width} height={node.height} rx="14"></rect>
          {#each bars as bar}
            <foreignObject x={bar.x} y={bar.y} width={bar.width} height={bar.height}>
              <div
                xmlns="http://www.w3.org/1999/xhtml"
                class={`graph-runtime-loading__node-bar-host graph-runtime-loading__node-bar-host--${bar.role}`}
              >
                <RuntimeLoadingBar width="100%" height="100%" />
              </div>
            </foreignObject>
          {/each}
        </g>
      {/each}
    </g>
  </svg>
</div>

<style>
  .graph-runtime-loading {
    position: absolute;
    inset: 0;
    z-index: 3;
    overflow: hidden;
    background:
      linear-gradient(90deg, rgba(148, 163, 184, 0.16) 1px, transparent 1px),
      linear-gradient(180deg, rgba(148, 163, 184, 0.14) 1px, transparent 1px),
      #f8fafc;
    background-size: 42px 42px, 42px 42px, auto;
    color: var(--text-primary);
    pointer-events: none;
  }

  .graph-runtime-loading__scene {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .graph-runtime-loading__edge {
    fill: none;
    stroke: rgba(99, 102, 241, 0.22);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .graph-runtime-loading__node {
    filter: drop-shadow(0 12px 36px rgba(15, 23, 42, 0.08));
  }

  .graph-runtime-loading__node-shell {
    fill: rgba(255, 255, 255, 0.9);
    stroke: rgba(148, 163, 184, 0.26);
    stroke-width: 1;
  }

  .graph-runtime-loading__node-bar-host {
    width: 100%;
    height: 100%;
    opacity: 0.95;
  }

  .graph-runtime-loading__node-bar-host--key {
    opacity: 0.88;
  }

  .graph-runtime-loading__node-bar-host--value {
    opacity: 0.97;
  }
</style>
