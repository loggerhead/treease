import type {
  EditorNavigationFacade as EditorNavigationFacadeContract,
  EditorNavigationOptions,
  GraphCellTarget,
  NavigationCommand,
  NavigationPath,
  NavigationResult,
  NavigationTarget,
  NavigationTargetReader,
  NavigationUserEvent,
} from './navigation-contract';
import type { NavigationEntitySlices, TabEntitySliceWriter } from './tab-navigation-store';

export type EditorSelectionCause =
  | 'user'
  | 'programmatic'
  | 'restore'
  | 'binding'
  | 'edit'
  | 'scroll'
  | 'unknown';

export type EditorSelectionCheckpoint = Readonly<{
  path: NavigationPath;
  cellTarget: GraphCellTarget;
}>;

/** Editor-owned, tab-local state. Runtime editor instances remain in TabRuntimeRegistry. */
export type EditorNavigationState = Readonly<{
  selection: EditorSelectionCheckpoint | null;
  /** Last valid selection fact consumed for navigation; unresolved Monaco ranges do not reset dedupe. */
  lastNavigationSelection: EditorSelectionCheckpoint | null;
}>;

export type EditorSelectionChange = Readonly<{
  target: NavigationTarget;
  cause: EditorSelectionCause;
  /** Null means the Monaco selection cannot be resolved to a semantic target. */
  path: NavigationPath | null;
  cellTarget: GraphCellTarget;
}>;

export type EditorRuntimeContext = Readonly<{
  target: NavigationTarget;
  /** Runtime adapters check immediately before every asynchronous editor write. */
  isCurrent: () => boolean;
  /** Background tabs may receive logical state, but cannot scroll or steal focus. */
  isVisible: boolean;
}>;

export type EditorLocateOptions = Readonly<{
  reveal: boolean;
  focus: boolean;
}>;

/** Binds a command to the editor runtime belonging to the command's captured tab. */
export interface EditorNavigationRuntimePort {
  locate(context: EditorRuntimeContext, command: NavigationCommand, options: EditorLocateOptions): Promise<NavigationResult>;
}

export type EditorSelectionUpdateResult =
  | NavigationResult
  | Readonly<{ kind: 'published'; event: Extract<NavigationUserEvent, { kind: 'editor-selection' }> }>;

type EditorFacadeOptions<Slices extends NavigationEntitySlices & { editorState: EditorNavigationState }> = Readonly<{
  writer: TabEntitySliceWriter<Slices, 'editorState'>;
  runtime: EditorNavigationRuntimePort;
  targetReader: NavigationTargetReader;
  isVisible: (target: NavigationTarget) => boolean;
  publish: (event: Extract<NavigationUserEvent, { kind: 'editor-selection' }>) => void;
}>;

function samePath(left: NavigationPath, right: NavigationPath): boolean {
  return left.length === right.length && left.every((segment, index) => JSON.stringify(segment) === JSON.stringify(right[index]));
}

function checkpoint(path: NavigationPath | null, cellTarget: GraphCellTarget): EditorSelectionCheckpoint | null {
  return path === null ? null : { path: [...path], cellTarget };
}

function freshness(targetReader: NavigationTargetReader, command: NavigationCommand): NavigationResult | null {
  const status = targetReader.status(command.target);
  if (status !== 'current') return { kind: status };
  return command.transaction.isCurrent() ? null : { kind: 'stale' };
}

/** Only a confirmed Monaco user selection can start cross-entity navigation. */
export function isNavigationSelectionCause(cause: EditorSelectionCause): boolean {
  return cause === 'user';
}

/**
 * Owns Editor selection checkpoints and the programmatic-selection loop boundary.
 * It never resolves a runtime from active-tab state and writes only editorState.
 */
export class TabEditorNavigationFacade<Slices extends NavigationEntitySlices & { editorState: EditorNavigationState }>
  implements EditorNavigationFacadeContract {
  constructor(private readonly options: EditorFacadeOptions<Slices>) {}

  recordSelection(change: EditorSelectionChange): EditorSelectionUpdateResult {
    const status = this.options.targetReader.status(change.target);
    if (status !== 'current') return { kind: status };

    const next = checkpoint(change.path, change.cellTarget);
    let navigationChanged = false;
    const result = this.options.writer.update(change.target, (state) => {
      navigationChanged = change.path !== null && !sameCheckpoint(state.lastNavigationSelection, next);
      const shouldRememberNavigation = change.path !== null && navigationChanged;
      if (sameCheckpoint(state.selection, next) && !shouldRememberNavigation) return state;
      return {
        selection: next,
        lastNavigationSelection: shouldRememberNavigation ? next : state.lastNavigationSelection,
      };
    });
    if (result.kind !== 'applied') return result;
    if (!isNavigationSelectionCause(change.cause) || change.path === null || !navigationChanged) return { kind: 'no-op' };

    const event = { kind: 'editor-selection', target: change.target, path: [...change.path], cellTarget: change.cellTarget } as const;
    this.options.publish(event);
    return { kind: 'published', event };
  }

  async navigate(command: NavigationCommand, options: EditorNavigationOptions): Promise<NavigationResult> {
    const before = freshness(this.options.targetReader, command);
    if (before) return before;

    const isVisible = this.options.isVisible(command.target);
    const context: EditorRuntimeContext = {
      target: command.target,
      isCurrent: () => freshness(this.options.targetReader, command) === null,
      isVisible,
    };
    const located = await this.invoke(context, command, { reveal: isVisible, focus: isVisible && options.focus });
    if (located.kind !== 'applied') return located;

    const after = freshness(this.options.targetReader, command);
    if (after) return after;
    const selection = checkpoint(command.path, command.cellTarget);
    const result = this.options.writer.update(command.target, () => ({ selection, lastNavigationSelection: selection }));
    return result.kind === 'applied' ? { kind: 'applied' } : result;
  }

  private async invoke(
    context: EditorRuntimeContext,
    command: NavigationCommand,
    locateOptions: EditorLocateOptions,
  ): Promise<NavigationResult> {
    const before = freshness(this.options.targetReader, command);
    if (before) return before;
    try {
      const result = await this.options.runtime.locate(context, command, locateOptions);
      return freshness(this.options.targetReader, command) ?? result;
    } catch (error) {
      return freshness(this.options.targetReader, command) ?? { kind: 'failed', error };
    }
  }
}

function sameCheckpoint(left: EditorSelectionCheckpoint | null, right: EditorSelectionCheckpoint | null): boolean {
  return left === right || (left !== null && right !== null && left.cellTarget === right.cellTarget && samePath(left.path, right.path));
}
