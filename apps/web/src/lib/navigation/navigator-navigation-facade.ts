import type {
  NavigationCommand,
  NavigationPath,
  NavigationResult,
  NavigationTarget,
  NavigationTargetReader,
  NavigatorHistoryMode,
  NavigatorNavigationFacade,
} from './navigation-contract';
import type { NavigationEntitySlices, TabEntitySliceWriter } from './tab-navigation-store';

export type NavigatorNavigationState = Readonly<{
  activePath: NavigationPath;
  history: readonly NavigationPath[];
  historyIndex: number;
  columnsMaterialized: boolean;
  expanded: boolean;
}>;

export type NavigatorPortCommand = Readonly<{
  target: NavigationTarget;
  transaction: NavigationCommand['transaction'];
  path: NavigationPath;
  history: readonly NavigationPath[];
  historyIndex: number;
  materializeColumns: boolean;
  expanded: boolean;
}>;

/** The runtime applies the navigator's path and history as one operation. */
export interface NavigatorNavigationPort {
  apply(command: NavigatorPortCommand): Promise<NavigationResult>;
}

type NavigatorCommand = NavigationCommand & Readonly<{ history: NavigatorHistoryMode }>;

type NavigatorFacadeOptions<Slices extends NavigationEntitySlices & { navigatorState: NavigatorNavigationState }> = Readonly<{
  writer: TabEntitySliceWriter<Slices, 'navigatorState'>;
  /** A read-only view of this facade's slice; it must not expose other entity state. */
  readState: (target: NavigationTarget) => NavigatorNavigationState | null;
  targetReader: NavigationTargetReader;
  port: NavigatorNavigationPort;
}>;

function freshness(targetReader: NavigationTargetReader, command: NavigationCommand): NavigationResult | null {
  const status = targetReader.status(command.target);
  if (status !== 'current') return { kind: status };
  return command.transaction.isCurrent() ? null : { kind: 'stale' };
}

function clonePath(path: NavigationPath): NavigationPath {
  return [...path];
}

function nextHistory(state: NavigatorNavigationState, path: NavigationPath, mode: NavigatorHistoryMode) {
  const current = state.history[state.historyIndex];
  const sameCurrent = current?.length === path.length && current.every((segment, index) => segment === path[index]);
  if (sameCurrent) return { history: state.history, historyIndex: state.historyIndex };

  if (mode === 'merge' && state.historyIndex >= 0) {
    const history = [...state.history];
    history[state.historyIndex] = clonePath(path);
    return { history, historyIndex: state.historyIndex };
  }

  const history = [...state.history.slice(0, state.historyIndex + 1), clonePath(path)];
  return { history, historyIndex: history.length - 1 };
}

/**
 * Tab-bound Column Navigator implementation. The port owns materialization;
 * this facade never expands or builds columns during a lightweight locate.
 */
export class TabNavigatorNavigationFacade<Slices extends NavigationEntitySlices & { navigatorState: NavigatorNavigationState }>
  implements NavigatorNavigationFacade {
  constructor(private readonly options: NavigatorFacadeOptions<Slices>) {}

  locate(command: NavigatorCommand): Promise<NavigationResult> {
    return this.apply(command, false);
  }

  navigate(command: NavigatorCommand): Promise<NavigationResult> {
    return this.apply(command, true);
  }

  private async apply(command: NavigatorCommand, materializeColumns: boolean): Promise<NavigationResult> {
    const before = freshness(this.options.targetReader, command);
    if (before) return before;

    const state = this.options.readState(command.target);
    if (!state) return freshness(this.options.targetReader, command) ?? { kind: 'stale' };
    const history = nextHistory(state, command.path, command.history);
    const next: NavigatorNavigationState = {
      activePath: clonePath(command.path),
      history: history.history,
      historyIndex: history.historyIndex,
      columnsMaterialized: materializeColumns ? true : state.columnsMaterialized,
      expanded: materializeColumns ? true : state.expanded,
    };

    const portResult = await this.options.port.apply({
      target: command.target,
      transaction: command.transaction,
      path: next.activePath,
      history: next.history,
      historyIndex: next.historyIndex,
      materializeColumns,
      expanded: next.expanded,
    });
    if (portResult.kind !== 'applied') return portResult;

    const after = freshness(this.options.targetReader, command);
    if (after) return after;
    const writeResult = this.options.writer.update(command.target, () => next);
    return writeResult.kind === 'applied' ? { kind: 'applied' } : writeResult;
  }
}
