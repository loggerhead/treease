import {
  type NavigationBehavior,
  type NavigationDispatchResult,
  type NavigationFacades,
  type NavigationResult,
  type NavigationSettings,
  type NavigationTarget,
  type NavigationTargetReader,
  type NavigationTransaction,
  type NavigationUserEvent,
  type PreviewCancellationCommand,
} from './navigation-contract';
import { navigationBehaviorPolicy } from './navigation-behavior-policy';

type CoordinatorOptions = Readonly<{
  facades: NavigationFacades;
  targetReader: NavigationTargetReader;
  getSettings: () => NavigationSettings;
}>;

function statusResult(targetReader: NavigationTargetReader, target: NavigationTarget): NavigationResult | null {
  const status = targetReader.status(target);
  return status === 'current' ? null : { kind: status };
}

function summarize(results: readonly NavigationResult[]): NavigationResult['kind'] {
  if (results.some((result) => result.kind === 'failed')) return 'failed';
  if (results.some((result) => result.kind === 'closed')) return 'closed';
  if (results.some((result) => result.kind === 'stale')) return 'stale';
  if (results.some((result) => result.kind === 'cancelled')) return 'cancelled';
  if (results.some((result) => result.kind === 'applied')) return 'applied';
  if (results.some((result) => result.kind === 'deferred')) return 'deferred';
  return 'no-op';
}

/** Workspace-level orchestration only: facade implementations own all entity state and runtime details. */
export class NavigationCoordinator {
  #nextTransactionId = 0;
  #latestTransactionByTab = new Map<string, number>();

  constructor(private readonly options: CoordinatorOptions) {}

  async dispatch(event: NavigationUserEvent): Promise<NavigationDispatchResult> {
    const behavior = navigationBehaviorPolicy.decide(event, this.options.getSettings());
    const beforeStart = statusResult(this.options.targetReader, event.target);
    if (beforeStart) return this.result(behavior, event.target, [beforeStart]);

    if (event.kind === 'graph-ready') {
      // Graph readiness is an observation, not user navigation. Reuse the
      // current transaction so a late scene commit cannot stale editor work.
      const transaction = this.capture(event.target);
      const graph = await this.options.facades.graph.flush({ target: event.target, transaction });
      return this.result(behavior, event.target, [graph]);
    }

    if (behavior === 'none') return this.result(behavior, event.target, [{ kind: 'no-op' }]);

    const transaction = this.begin(event.target);
    const results = await this.consume(event, behavior, transaction);
    const staleResult = statusResult(this.options.targetReader, event.target);
    return this.result(behavior, event.target, transaction.isCurrent() ? results : results.map(() => staleResult ?? { kind: 'stale' }));
  }

  private begin(target: NavigationTarget): NavigationTransaction {
    const id = ++this.#nextTransactionId;
    this.#latestTransactionByTab.set(target.tabId, id);
    return {
      id,
      target,
      isCurrent: () => this.#latestTransactionByTab.get(target.tabId) === id && this.options.targetReader.status(target) === 'current',
    };
  }

  private capture(target: NavigationTarget): NavigationTransaction {
    const id = this.#latestTransactionByTab.get(target.tabId) ?? ++this.#nextTransactionId;
    if (!this.#latestTransactionByTab.has(target.tabId)) this.#latestTransactionByTab.set(target.tabId, id);
    return {
      id,
      target,
      isCurrent: () => this.#latestTransactionByTab.get(target.tabId) === id && this.options.targetReader.status(target) === 'current',
    };
  }

  private async consume(
    event: NavigationUserEvent,
    behavior: Exclude<NavigationBehavior, 'none'>,
    transaction: NavigationTransaction,
  ): Promise<readonly NavigationResult[]> {
    if (!transaction.isCurrent()) return [statusResult(this.options.targetReader, event.target) ?? { kind: 'stale' }];

    if (event.kind === 'search-cancel') {
      const command: PreviewCancellationCommand = { target: event.target, transaction, previewId: event.previewId };
      return Promise.all([
        this.options.facades.graph.cancelPreview(command),
        this.options.facades.search.endPreview({ ...command, reason: 'cancelled' }),
      ]);
    }

    if (!('path' in event)) return [{ kind: 'no-op' }];

    const command = { target: event.target, transaction, path: event.path, cellTarget: event.cellTarget };
    if (event.kind === 'search-preview') {
      const preview = { ...command, previewId: event.previewId } as const;
      const begun = this.options.facades.search.beginPreview(preview);
      if (begun.kind !== 'applied' && begun.kind !== 'no-op') return [begun];
      return Promise.all([
        Promise.resolve(begun),
        this.options.facades.graph.preview({
          ...preview,
          mode: behavior === 'graph-viewport-preview' ? 'viewport' : 'highlight',
        }),
      ]);
    }

    if (behavior === 'locate') {
      return Promise.all([
        this.options.facades.graph.locate(command),
        this.options.facades.navigator.locate({ ...command, history: event.kind === 'editor-selection' ? 'merge' : 'push' }),
        this.options.facades.search.discardPreview({ target: event.target, transaction, reason: 'superseded' }),
      ]);
    }

    const previewEnd = event.kind === 'search-commit'
      ? this.options.facades.search.endPreview({ target: event.target, transaction, previewId: event.previewId, reason: 'committed' })
      : Promise.resolve<NavigationResult>({ kind: 'no-op' });
    // Commit the Navigator's atomic path/history projection before asynchronous
    // viewport work. A graph reveal can remount the visual scene; it must never
    // race or supersede the navigation state it was asked to render.
    const navigator = await this.options.facades.navigator.navigate({ ...command, history: 'push' });
    if (!transaction.isCurrent()) return [navigator];
    return [
      navigator,
      ...(await Promise.all([
        this.options.facades.editor.navigate(command, { focus: event.kind === 'editor-selection' }),
        this.options.facades.graph.navigate(command),
        previewEnd,
        event.kind === 'search-commit'
          ? Promise.resolve<NavigationResult>({ kind: 'no-op' })
          : this.options.facades.search.discardPreview({ target: event.target, transaction, reason: 'superseded' }),
      ])),
    ];
  }

  private result(
    behavior: NavigationBehavior,
    target: NavigationTarget,
    results: readonly NavigationResult[],
  ): NavigationDispatchResult {
    return { behavior, target, results, outcome: summarize(results) };
  }
}
