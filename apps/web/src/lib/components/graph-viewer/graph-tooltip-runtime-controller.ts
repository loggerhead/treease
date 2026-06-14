import { TooltipPlugin } from '../../leafer-x-tooltip';

export function createGraphTooltipRuntimeController(options: {
  hoverPanelController: {
    ensureTooltipRuntime: () => Promise<void>;
    hasTooltipPanelActivity: () => boolean;
    resolveGraphHoverPreviewTarget: (target: unknown) => unknown;
    renderTooltipContent: (host: Element, target: unknown) => Promise<void>;
    clearTooltipPreviewHost: (host: Element) => void;
    destroyTooltipPanelRuntime: () => void;
  };
  resolveTooltipHoverTarget: (target: unknown) => unknown;
  getTooltipContent: (target: unknown) => string;
  hasActiveEdit: () => boolean;
  isFullEditInteractionBlocked: () => boolean;
  setRuntimeHoverPanelDebugState: (state: unknown) => void;
}) {
  let plugin: TooltipPlugin | null = null;

  return {
    ensureTooltipRuntime() {
      return options.hoverPanelController.ensureTooltipRuntime();
    },
    attach(app: unknown, events: { LeaferEvent?: unknown; PointerEvent?: unknown }) {
      plugin?.destroy?.();
      plugin = new TooltipPlugin(app as any, {
        className: 'leafer-x-tooltip',
        includeTypes: ['Text', 'Box'],
        closeDelay: 320,
        resolveNode: (node) => options.resolveTooltipHoverTarget(node ?? null),
        interactive: (node) =>
          !!options.hoverPanelController.resolveGraphHoverPreviewTarget(
            options.resolveTooltipHoverTarget(node ?? null),
          ),
        shouldKeepOpen: () => options.hoverPanelController.hasTooltipPanelActivity() || options.hasActiveEdit(),
        events: events as any,
        shouldBegin: (_event: unknown, node?: unknown) => {
          if (options.isFullEditInteractionBlocked()) return false;
          const target = node ?? null;
          const cell = (target as { __graphCell?: unknown } | null)?.__graphCell ?? null;
          if (!cell) {
            options.setRuntimeHoverPanelDebugState({ phase: 'should-begin-no-cell', error: '' });
            return false;
          }
          const kind = (target as { __graphCellKind?: string } | null)?.__graphCellKind;
          if (kind === 'header') {
            options.setRuntimeHoverPanelDebugState({ phase: 'should-begin-header', error: '' });
            return false;
          }
          if (
            options.hoverPanelController.resolveGraphHoverPreviewTarget(
              options.resolveTooltipHoverTarget(target),
            )
          ) {
            options.setRuntimeHoverPanelDebugState({ phase: 'should-begin-pass', error: '' });
            return true;
          }
          options.setRuntimeHoverPanelDebugState({ phase: 'should-begin-preview-miss', error: '' });
          return false;
        },
        getContent: (target: unknown) => options.getTooltipContent(target ?? null),
        onOpen: (host, target) => {
          void options.hoverPanelController.renderTooltipContent(host, target ?? null);
        },
        onUpdate: () => {},
        onClose: (host) => {
          options.hoverPanelController.clearTooltipPreviewHost(host);
          options.hoverPanelController.destroyTooltipPanelRuntime();
        },
      });
    },
    refreshVisibility() {
      plugin?.refreshVisibility?.();
    },
    refreshPosition() {
      plugin?.refreshPosition?.();
    },
    destroy() {
      options.hoverPanelController.destroyTooltipPanelRuntime();
      plugin?.destroy?.();
      plugin = null;
    },
  };
}
