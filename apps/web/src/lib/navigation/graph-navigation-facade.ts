import type {
  GraphNavigationFacade as GraphNavigationFacadeContract,
  GraphPreviewCommand,
  NavigationCommand,
  NavigationResult,
  NavigationTarget,
  NavigationTargetReader,
  PreviewCancellationCommand,
} from './navigation-contract';

export type GraphPreviewBaseline = Readonly<{
  selection: unknown;
  viewport: unknown;
}>;

export type GraphRuntimeContext = Readonly<{
  target: NavigationTarget;
  /** Runtime adapters must check this immediately before each asynchronous write. */
  isCurrent: () => boolean;
}>;

/**
 * Production adapters bind these operations to a graph scene for one captured tab.
 * They must not resolve a scene from active-tab state.
 */
export interface GraphNavigationRuntimePort {
  capturePreviewBaseline(context: GraphRuntimeContext): Promise<GraphPreviewBaseline>;
  highlight(context: GraphRuntimeContext, command: NavigationCommand): Promise<NavigationResult>;
  reveal(context: GraphRuntimeContext, command: NavigationCommand): Promise<NavigationResult>;
  restoreSelection(context: GraphRuntimeContext, baseline: GraphPreviewBaseline): Promise<NavigationResult>;
  restoreViewport(context: GraphRuntimeContext, baseline: GraphPreviewBaseline): Promise<NavigationResult>;
  cancelViewportTransition(context: GraphRuntimeContext): Promise<NavigationResult>;
}

type PreviewSession = Readonly<{
  target: NavigationTarget;
  previewId: string;
  baseline: GraphPreviewBaseline;
  ownsViewport: boolean;
}>;

type GraphNavigationFacadeOptions = Readonly<{
  runtime: GraphNavigationRuntimePort;
  targetReader: NavigationTargetReader;
}>;

function targetKey(target: NavigationTarget): string {
  return `${target.tabId}:${target.documentKey}:${target.generation}:${target.revision}`;
}

function sameTarget(left: NavigationTarget, right: NavigationTarget): boolean {
  return left.tabId === right.tabId
    && left.documentKey === right.documentKey
    && left.generation === right.generation
    && left.revision === right.revision;
}

function completed(result: NavigationResult): boolean {
  return result.kind === 'applied' || result.kind === 'no-op';
}

function combined(first: NavigationResult, second: NavigationResult): NavigationResult {
  return first.kind === 'applied' || second.kind === 'applied' ? { kind: 'applied' } : second;
}

/** Owns Graph-only preview baseline and viewport-transition freshness. */
export class GraphNavigationFacade implements GraphNavigationFacadeContract {
  private readonly previews = new Map<string, PreviewSession>();

  constructor(private readonly options: GraphNavigationFacadeOptions) {}

  async locate(command: NavigationCommand): Promise<NavigationResult> {
    const context = this.context(command);
    const invalid = this.invalid(context);
    if (invalid) return invalid;
    await this.discardPreview(command, context);
    return this.invoke(context, () => this.options.runtime.highlight(context, command));
  }

  async navigate(command: NavigationCommand): Promise<NavigationResult> {
    const context = this.context(command);
    const invalid = this.invalid(context);
    if (invalid) return invalid;
    await this.discardPreview(command, context);
    const highlight = await this.invoke(context, () => this.options.runtime.highlight(context, command));
    if (!completed(highlight)) return highlight;
    return combined(highlight, await this.invoke(context, () => this.options.runtime.reveal(context, command)));
  }

  async preview(command: GraphPreviewCommand): Promise<NavigationResult> {
    const context = this.context(command);
    const invalid = this.invalid(context);
    if (invalid) return invalid;

    const key = targetKey(command.target);
    let session = this.previews.get(key);
    if (!session) {
      const baseline = await this.captureBaseline(context);
      const afterCapture = this.invalid(context);
      if (afterCapture) return afterCapture;
      if (!baseline) return { kind: 'failed', error: new Error('Graph preview baseline was unavailable') };
      session = { target: command.target, previewId: command.previewId, baseline, ownsViewport: command.mode === 'viewport' };
      this.previews.set(key, session);
    } else {
      // A newer result inherits the original baseline; restoring an intermediate preview would be wrong.
      session = { ...session, previewId: command.previewId, ownsViewport: session.ownsViewport || command.mode === 'viewport' };
      this.previews.set(key, session);
    }

    const highlight = await this.invoke(context, () => this.options.runtime.highlight(context, command));
    if (!completed(highlight) || command.mode === 'highlight') return highlight;
    return combined(highlight, await this.invoke(context, () => this.options.runtime.reveal(context, command)));
  }

  async cancelPreview(command: PreviewCancellationCommand): Promise<NavigationResult> {
    const context = this.context(command);
    const invalid = this.invalid(context);
    if (invalid) return invalid;
    const key = targetKey(command.target);
    const session = this.previews.get(key);
    if (!session || session.previewId !== command.previewId) return { kind: 'no-op' };

    const selection = await this.invoke(context, () => this.options.runtime.restoreSelection(context, session.baseline));
    if (!completed(selection)) return selection;
    const viewport = session.ownsViewport
      ? await this.invoke(context, () => this.options.runtime.restoreViewport(context, session.baseline))
      : { kind: 'applied' } as const;
    if (completed(viewport) && this.previews.get(key) === session) this.previews.delete(key);
    return combined(selection, viewport);
  }

  /** Called by the Graph gesture handler; a manual viewport change revokes only viewport restore ownership. */
  async releasePreviewViewport(target: NavigationTarget): Promise<NavigationResult> {
    const status = this.options.targetReader.status(target);
    if (status !== 'current') return { kind: status };
    const key = targetKey(target);
    const session = this.previews.get(key);
    if (!session || !session.ownsViewport) return { kind: 'no-op' };
    const context: GraphRuntimeContext = { target, isCurrent: () => this.options.targetReader.status(target) === 'current' };
    const cancelled = await this.invoke(context, () => this.options.runtime.cancelViewportTransition(context));
    if (cancelled.kind === 'applied' && this.previews.get(key) === session) {
      this.previews.set(key, { ...session, ownsViewport: false });
    }
    return cancelled;
  }

  private context(command: Pick<NavigationCommand, 'target' | 'transaction'>): GraphRuntimeContext {
    return { target: command.target, isCurrent: () => command.transaction.isCurrent() && this.options.targetReader.status(command.target) === 'current' };
  }

  private invalid(context: GraphRuntimeContext): NavigationResult | null {
    const status = this.options.targetReader.status(context.target);
    if (status !== 'current') return { kind: status };
    return context.isCurrent() ? null : { kind: 'stale' };
  }

  private async captureBaseline(context: GraphRuntimeContext): Promise<GraphPreviewBaseline | null> {
    try {
      return await this.options.runtime.capturePreviewBaseline(context);
    } catch (error) {
      return null;
    }
  }

  private async discardPreview(command: NavigationCommand, context: GraphRuntimeContext): Promise<void> {
    const key = targetKey(command.target);
    const session = this.previews.get(key);
    if (!session || !sameTarget(session.target, command.target)) return;
    this.previews.delete(key);
    await this.invoke(context, () => this.options.runtime.cancelViewportTransition(context));
  }

  private async invoke(context: GraphRuntimeContext, operation: () => Promise<NavigationResult>): Promise<NavigationResult> {
    const before = this.invalid(context);
    if (before) return before;
    try {
      const result = await operation();
      return this.invalid(context) ?? result;
    } catch (error) {
      return this.invalid(context) ?? { kind: 'failed', error };
    }
  }
}
