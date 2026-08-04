import type {
  NavigationResult,
  NavigationScopeCommand,
  NavigationTarget,
  NavigationTargetReader,
  PreviewCancellationCommand,
  PreviewEndReason,
  SearchNavigationFacade,
} from './navigation-contract';
import type { NavigationEntitySlices, TabEntitySliceWriter } from './tab-navigation-store';

export type SearchNavigationState = Readonly<{ previewId: string | null }>;

type SearchFacadeOptions<Slices extends NavigationEntitySlices & { searchState: SearchNavigationState }> = Readonly<{
  writer: TabEntitySliceWriter<Slices, 'searchState'>;
  readState: (target: NavigationTarget) => SearchNavigationState | null;
  targetReader: NavigationTargetReader;
}>;

function freshness(targetReader: NavigationTargetReader, command: NavigationScopeCommand): NavigationResult | null {
  const status = targetReader.status(command.target);
  if (status !== 'current') return { kind: status };
  return command.transaction.isCurrent() ? null : { kind: 'stale' };
}

/** Search UI identity is tab-local; Graph preview state deliberately stays outside this facade. */
export class TabSearchNavigationFacade<Slices extends NavigationEntitySlices & { searchState: SearchNavigationState }>
  implements SearchNavigationFacade {
  constructor(private readonly options: SearchFacadeOptions<Slices>) {}

  beginPreview(command: PreviewCancellationCommand): NavigationResult {
    const before = freshness(this.options.targetReader, command);
    if (before) return before;
    if (this.options.readState(command.target)?.previewId === command.previewId) return { kind: 'no-op' };
    const result = this.options.writer.update(command.target, () => ({ previewId: command.previewId }));
    return result.kind === 'applied' ? { kind: 'applied' } : result;
  }

  async endPreview(command: PreviewCancellationCommand & Readonly<{ reason: PreviewEndReason }>): Promise<NavigationResult> {
    const before = freshness(this.options.targetReader, command);
    if (before) return before;
    if (this.options.readState(command.target)?.previewId !== command.previewId) return { kind: 'no-op' };
    const result = this.options.writer.update(command.target, () => ({ previewId: null }));
    return result.kind === 'applied' ? { kind: 'applied' } : result;
  }

  async discardPreview(command: NavigationScopeCommand & Readonly<{ reason: 'superseded' }>): Promise<NavigationResult> {
    const before = freshness(this.options.targetReader, command);
    if (before) return before;
    if (this.options.readState(command.target)?.previewId === null) return { kind: 'no-op' };
    const result = this.options.writer.update(command.target, () => ({ previewId: null }));
    return result.kind === 'applied' ? { kind: 'applied' } : result;
  }
}
