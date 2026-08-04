import type { PathSeg } from '@core-wasm/index';

/** A navigation target is captured when a user fact occurs; it is never inferred from active tab state. */
export type NavigationTarget = Readonly<{
  workspaceId: string;
  tabId: string;
  documentKey: string;
  generation: number;
  revision: number;
}>;

export type NavigationPath = readonly PathSeg[];
export type GraphCellTarget = 'key' | 'value' | 'node';

export type NavigationResult =
  | { kind: 'applied' }
  | { kind: 'no-op' }
  | { kind: 'stale' }
  | { kind: 'cancelled' }
  | { kind: 'closed' }
  | { kind: 'failed'; error: unknown };

export type NavigationTargetStatus = 'current' | 'stale' | 'closed';

/** Supplied by TabStore/TabRuntimeRegistry integration, not implemented by the coordinator. */
export interface NavigationTargetReader {
  status(target: NavigationTarget): NavigationTargetStatus;
}

export interface NavigationTransaction {
  readonly id: number;
  readonly target: NavigationTarget;
  /** False as soon as a newer navigation starts for this tab or its target becomes invalid. */
  isCurrent(): boolean;
}

export type NavigationCommand = Readonly<{
  target: NavigationTarget;
  transaction: NavigationTransaction;
  path: NavigationPath;
  cellTarget: GraphCellTarget;
}>;

/** Focus follows the entity that originated navigation; linked editor reveals must not steal it. */
export type EditorNavigationOptions = Readonly<{
  focus: boolean;
}>;

/** Search owns the UI identity; Graph uses it only to protect its preview baseline. */
export type GraphPreviewCommand = NavigationCommand & Readonly<{
  previewId: string;
  mode: 'highlight' | 'viewport';
}>;

export type NavigationScopeCommand = Readonly<{
  target: NavigationTarget;
  transaction: NavigationTransaction;
}>;

export interface EditorNavigationFacade {
  navigate(command: NavigationCommand, options: EditorNavigationOptions): Promise<NavigationResult>;
}

export interface GraphNavigationFacade {
  locate(command: NavigationCommand): Promise<NavigationResult>;
  navigate(command: NavigationCommand): Promise<NavigationResult>;
  preview(command: GraphPreviewCommand): Promise<NavigationResult>;
  cancelPreview(command: PreviewCancellationCommand): Promise<NavigationResult>;
}

export type NavigatorHistoryMode = 'merge' | 'push';

export interface NavigatorNavigationFacade {
  locate(command: NavigationCommand & Readonly<{ history: NavigatorHistoryMode }>): Promise<NavigationResult>;
  navigate(command: NavigationCommand & Readonly<{ history: NavigatorHistoryMode }>): Promise<NavigationResult>;
}

export type PreviewEndReason = 'cancelled' | 'committed' | 'superseded';

export type PreviewCancellationCommand = Readonly<{
  target: NavigationTarget;
  transaction: NavigationTransaction;
  previewId: string;
}>;

export interface SearchNavigationFacade {
  beginPreview(command: PreviewCancellationCommand): NavigationResult;
  endPreview(command: PreviewCancellationCommand & Readonly<{ reason: PreviewEndReason }>): Promise<NavigationResult>;
  /** Clears UI preview identity when another entity begins a newer navigation for this tab. */
  discardPreview(command: NavigationScopeCommand & Readonly<{ reason: 'superseded' }>): Promise<NavigationResult>;
}

export type NavigationUserEvent =
  | Readonly<{ kind: 'editor-selection'; target: NavigationTarget; path: NavigationPath; cellTarget: GraphCellTarget }>
  | Readonly<{ kind: 'graph-cell'; target: NavigationTarget; path: NavigationPath; cellTarget: GraphCellTarget }>
  | Readonly<{ kind: 'navigator-column'; target: NavigationTarget; path: NavigationPath; cellTarget: GraphCellTarget }>
  | Readonly<{ kind: 'navigator-tree-path'; target: NavigationTarget; path: NavigationPath; cellTarget: GraphCellTarget }>
  | Readonly<{ kind: 'search-preview'; target: NavigationTarget; path: NavigationPath; cellTarget: GraphCellTarget; previewId: string }>
  | Readonly<{ kind: 'search-commit'; target: NavigationTarget; path: NavigationPath; cellTarget: GraphCellTarget; previewId: string }>
  | Readonly<{ kind: 'search-cancel'; target: NavigationTarget; previewId: string }>
  | Readonly<{ kind: 'graph-viewport-gesture'; target: NavigationTarget }>
  | Readonly<{ kind: 'editor-edit'; target: NavigationTarget }>
  | Readonly<{ kind: 'editor-scroll'; target: NavigationTarget }>
  | Readonly<{ kind: 'tab-activated'; target: NavigationTarget }>
  | Readonly<{ kind: 'state-restored'; target: NavigationTarget }>;

export type NavigationBehavior =
  | 'none'
  | 'locate'
  | 'navigate'
  | 'graph-highlight-preview'
  | 'graph-viewport-preview'
  | 'cancel-preview';

export type NavigationSettings = Readonly<{ completeNavigationEnabled: boolean }>;

export interface NavigationBehaviorPolicy {
  decide(event: NavigationUserEvent, settings: NavigationSettings): NavigationBehavior;
}

export type NavigationFacades = Readonly<{
  editor: EditorNavigationFacade;
  graph: GraphNavigationFacade;
  navigator: NavigatorNavigationFacade;
  search: SearchNavigationFacade;
}>;

export type NavigationDispatchResult = Readonly<{
  behavior: NavigationBehavior;
  target: NavigationTarget;
  results: readonly NavigationResult[];
  outcome: NavigationResult['kind'];
}>;
