// 职责：Graph hover 控制器占位：graph 不再通过 hover 展示预览，但保留现有 runtime 接缝
import type { GraphViewerConfig } from '../../settings/ui-settings';
import type { PathSeg } from '../../store/tree-path';
import type { GraphCell, GraphCellKind } from '../../graph/graph-viewer-render';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import type { LeaferEditor, LeaferText } from './model';
import type { GraphRuntimeHoverPanelDebugState, GraphRuntimeHoverPreviewState } from './runtime/scene-types';
import type {
  GraphHoverPreviewTarget,
  TooltipPanelPrewarmDebugSnapshot,
} from './graph-hover-panel-types';

export type { GraphHoverPreviewKind, GraphHoverPreviewTarget } from './graph-hover-panel-types';

type HoverPanelControllerDeps = {
  [key: string]: unknown;
  getLanguageId: () => SupportedEditorLanguageId;
  getRenderConfig: () => GraphViewerConfig;
  bindGraphEditorLifecycle: (editor: LeaferEditor | null) => void;
  canOpenSubgraphPreviewForCell: (cell: GraphCell, target: 'key' | 'value' | 'node') => boolean;
  setRuntimeHoverPreviewState: (preview: GraphRuntimeHoverPreviewState | null) => void;
  setRuntimeHoverPanelDebugState: (state: GraphRuntimeHoverPanelDebugState) => void;
  refreshTooltipPosition: () => void;
};

function resolveGraphHoverPanelTarget(kind: GraphCellKind | null): 'key' | 'value' | 'node' | null {
  if (kind === 'key') return 'key';
  if (kind === 'value') return 'value';
  if (kind === 'meta' || kind === 'header') return 'node';
  return null;
}

export function isGraphHoverTargetOverflowing(target: LeaferText | null): boolean {
  return Boolean(target && 'isOverflow' in target && (target as { isOverflow?: boolean }).isOverflow);
}

type GraphHoverPreviewCandidate = Pick<LeaferText, '__graphCell' | '__graphCellKind' | '__graphNodeKind'> & {
  isOverflow?: boolean;
};

export function canOpenSubgraphPreviewForCell(cell: GraphCell, target: 'key' | 'value' | 'node'): boolean {
  if (cell.isHeader) return false;
  if (target === 'key') return true;
  if (target === 'node') return !!cell.path?.length;
  if (cell.valueType !== 'object' && cell.valueType !== 'array') return true;
  if (!cell.isTableCell) return false;
  if (cell.isHeaderlessTable && !cell.isScrollableTable) return false;
  if (cell.text === '{}' || cell.text === '[]') return false;
  return true;
}

export function resolveGraphHoverPreviewRule(
  _target: GraphHoverPreviewCandidate | null,
  _canOpenSubgraphPreviewForCell: (cell: GraphCell, target: 'key' | 'value' | 'node') => boolean,
): GraphHoverPreviewTarget | null {
  return null;
}

export function createGraphHoverPanelController(deps: HoverPanelControllerDeps) {
  function ensureTooltipRuntime(): Promise<void> {
    return Promise.resolve();
  }

  function destroyTooltipPanelRuntime(): void {
    deps.setRuntimeHoverPreviewState(null);
  }

  function disposeTooltipEditor(): void {
    destroyTooltipPanelRuntime();
  }

  function applyTheme(settings: unknown): void {
    void settings;
    void deps.getRenderConfig();
  }

  function resolveGraphHoverPreviewTarget(target: LeaferText | null): GraphHoverPreviewTarget | null {
    return resolveGraphHoverPreviewRule(target as GraphHoverPreviewCandidate | null, deps.canOpenSubgraphPreviewForCell);
  }

  function scheduleTooltipPanelPrewarm(): void {}

  function clearTooltipPreviewHost(host: HTMLElement): void {
    void host;
    deps.setRuntimeHoverPreviewState(null);
  }

  async function renderTooltipContent(host: HTMLElement, target: LeaferText | null): Promise<void> {
    void host;
    const previewTarget = resolveGraphHoverPreviewTarget(target);
    deps.setRuntimeHoverPreviewState(
      previewTarget
        ? {
            kind: 'pre',
            text: previewTarget.cell.text,
            language: deps.getLanguageId(),
            visible: true,
          }
        : null,
    );
    deps.setRuntimeHoverPanelDebugState({ phase: previewTarget ? 'preview-ready' : 'preview-empty', error: '' });
    deps.refreshTooltipPosition();
  }

  function hasTooltipPanelActivity(): boolean {
    return false;
  }

  function getTooltipPanelApp() {
    return null;
  }

  function getTooltipPanelClickTargets() {
    return [];
  }

  function getTooltipPanelPath(): PathSeg[] {
    return [];
  }

  function getTooltipPanelPrewarmDebugSnapshot(): TooltipPanelPrewarmDebugSnapshot {
    return {
      scheduledPaths: [],
      completedPaths: [],
      inFlightPath: null,
    };
  }

  deps.bindGraphEditorLifecycle(null);

  return {
    applyTheme,
    clearTooltipPreviewHost,
    destroyTooltipPanelRuntime,
    disposeTooltipEditor,
    ensureTooltipRuntime,
    getTooltipPanelApp,
    getTooltipPanelClickTargets,
    getTooltipPanelPath,
    getTooltipPanelPrewarmDebugSnapshot,
    hasTooltipPanelActivity,
    renderTooltipContent,
    resolveGraphHoverPreviewTarget,
    scheduleTooltipPanelPrewarm,
  };
}
