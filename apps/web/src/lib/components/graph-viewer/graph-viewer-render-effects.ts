// Responsibility: schedule document-graph rendering and deduplicate GraphViewer render effects.
import {
  createFullEditExternalRenderAuthority,
  type FullEditExternalRenderSessionRef,
} from '../../graph-stream/full-edit-render-authority';
import type { FullEditUiState, JsonBlockSelection } from '../../store/full-edit-ui-store';
import type { FullEditGraphRenderOwnership } from './graph-view-runtime-lifecycle';

type RenderDocumentGraph = (input: {
  kind: 'incremental' | 'full-edit';
  documentKey: string;
  language: string;
  text: string;
  revision: number;
}) => Promise<unknown>;

type AttachFullEditDocumentJobSession = (input: FullEditExternalRenderSessionRef) => Promise<unknown | null>;


type RenderEffectsDeps = {
  shouldAttachGraphViewerTestHooks: () => boolean;
  getGraphStreamState: () => Record<string, any> | null;
  replaceGraphStreamState: (state: Record<string, any>) => void;
  renderDocumentGraph: RenderDocumentGraph;
  attachFullEditDocumentJobSession: AttachFullEditDocumentJobSession;
  renderJsonBlockSelection: (selection: JsonBlockSelection) => Promise<unknown>;
  markGraphRequested: (input: {
    documentKey: string;
    revision: number;
    mode: 'committed' | 'streaming' | 'json-block';
  }) => void;
  resetStreamProgress: () => void;
  onStreamingRenderError: (error: unknown) => void;
};

type FullEditSessionSnapshot = {
  hasRenderRuntime: boolean;
  documentKey: string;
  language: string;
  sourceText: string;
};

type IncrementalRenderSnapshot = {
  hasRenderRuntime: boolean;
  isBlocked: boolean;
  documentKey: string;
  language: string;
  sourceText: string;
  editorRevision: number;
  graphAppliedRevision: number;
};

export function createGraphViewerRenderEffects(deps: RenderEffectsDeps) {
  let pendingRenderSignature = '';
  let graphRenderDelayHandle: number | null = null;
  let latestIncrementalSnapshot: IncrementalRenderSnapshot | null = null;
  let lastEditorRevisionRendered = -1;
  let lastRenderedDocumentKey = '';
  let lastRenderedSourceText = '';
  let lastRenderedLanguageId = '';
  let lastFullEditRenderSignature = '';
  const externalFullEditRenderAuthority = createFullEditExternalRenderAuthority();
  let pendingJsonBlockSignature = '';
  let lastJsonBlockActive = false;

  function markRendered(documentKey: string, revision: number, sourceText: string, language: string): void {
    lastEditorRevisionRendered = revision;
    lastRenderedDocumentKey = documentKey;
    lastRenderedSourceText = sourceText;
    lastRenderedLanguageId = language;
    pendingRenderSignature = '';
    externalFullEditRenderAuthority.markRendered(documentKey, revision, language);
  }

  function maybeAttachFullEditSession(
    fullEditUiState: FullEditUiState | null | undefined,
    snapshot: FullEditSessionSnapshot,
  ): FullEditGraphRenderOwnership {
    const documentKey = fullEditUiState?.documentKey ?? snapshot.documentKey;
    const language = fullEditUiState?.language || snapshot.language;
    const renderSignature = `${documentKey}|${fullEditUiState?.revision ?? -1}|${language}|${snapshot.sourceText}`;
    const ownership = {
      kind: 'started',
      documentKey,
      revision: fullEditUiState?.revision ?? -1,
    } as const;
    if (snapshot.documentKey === '' || fullEditUiState?.active !== true) return { kind: 'not-started' };
    if (fullEditUiState.phase === 'preparing' || fullEditUiState.phase === 'idle') return { kind: 'not-started' };
    if (fullEditUiState.transportKind === 'file') {
      if (!fullEditUiState.sessionId) return { kind: 'not-started' };
      if (
        externalFullEditRenderAuthority.hasActiveRender(documentKey, fullEditUiState.revision, language) ||
        externalFullEditRenderAuthority.hasCompletedRender(documentKey, fullEditUiState.revision, language)
      ) {
        return ownership;
      }
      // Without a ready View Runtime there is no graph request to own; the lifecycle must leave
      // the revision available to the canonical incremental path once Full Edit ends.
      if (!snapshot.hasRenderRuntime) return { kind: 'not-started' };
      deps.markGraphRequested({
        documentKey,
        revision: fullEditUiState.revision,
        mode: 'streaming',
      });
      const externalRender = externalFullEditRenderAuthority.claim({
        sessionId: fullEditUiState.sessionId,
        documentKey,
        language,
        revision: fullEditUiState.revision,
      });
      if (!externalRender) return { kind: 'not-started' };
      void deps
        .attachFullEditDocumentJobSession(externalRender)
        .then((result) => {
          if (result == null) {
            externalFullEditRenderAuthority.release(externalRender);
          }
        })
        .catch((error) => {
          externalFullEditRenderAuthority.release(externalRender);
          deps.onStreamingRenderError(error);
        });
      return ownership;
    }
    if (renderSignature === lastFullEditRenderSignature) return ownership;
    // Do not record ownership for a request that cannot reach the View Runtime yet.
    if (!snapshot.hasRenderRuntime) return { kind: 'not-started' };
    lastFullEditRenderSignature = renderSignature;
    deps.markGraphRequested({
      documentKey,
      revision: fullEditUiState.revision,
      mode: 'streaming',
    });
    void deps.renderDocumentGraph({
      kind: 'full-edit',
      documentKey,
      language,
      text: snapshot.sourceText,
      revision: fullEditUiState.revision,
    }).catch(deps.onStreamingRenderError);
    return ownership;
  }

  function maybeRenderIncremental(snapshot: IncrementalRenderSnapshot): void {
    latestIncrementalSnapshot = snapshot;
    if (lastJsonBlockActive && !snapshot.isBlocked) {
      pendingJsonBlockSignature = '';
      pendingRenderSignature = '';
      lastJsonBlockActive = false;
    }
    if (!snapshot.hasRenderRuntime || snapshot.documentKey === '' || snapshot.isBlocked) return;
    const renderText = snapshot.sourceText;
    const documentKey = snapshot.documentKey;
    const revision = snapshot.editorRevision;
    const language = snapshot.language;
    if (externalFullEditRenderAuthority.hasActiveRender(documentKey, revision, language)) return;
    if (externalFullEditRenderAuthority.hasCompletedRender(documentKey, revision, language)) {
      markRendered(documentKey, revision, renderText, language);
      return;
    }
    if (deps.shouldAttachGraphViewerTestHooks()) {
      const previous = deps.getGraphStreamState();
      deps.replaceGraphStreamState({
        ...(previous ?? { partialSeen: false, finalSeen: false }),
        reactiveRenderCalls: (previous?.reactiveRenderCalls ?? 0) + 1,
        lastReactiveRenderTextLength: renderText.length,
        lastReactiveDocumentKey: documentKey,
        lastReactiveRevision: revision,
        lastReactiveLanguage: language,
        lastPhase: 'reactive-entered',
      });
    }
    if (
      documentKey === lastRenderedDocumentKey &&
      revision === lastEditorRevisionRendered &&
      renderText === lastRenderedSourceText &&
      language === lastRenderedLanguageId
    ) return;
    const renderSignature = `${documentKey}|${revision}|${language}|${renderText}`;
    const graphAlreadyUpToDate =
      snapshot.graphAppliedRevision >= revision &&
      lastRenderedDocumentKey === documentKey &&
      lastEditorRevisionRendered === revision &&
      lastRenderedSourceText === renderText &&
      lastRenderedLanguageId === language;
    if (graphAlreadyUpToDate) {
      markRendered(documentKey, revision, renderText, language);
      return;
    }
    if (pendingRenderSignature === renderSignature) return;
    pendingRenderSignature = renderSignature;
    deps.markGraphRequested({
      documentKey,
      revision,
      mode: 'committed',
    });
    if (graphRenderDelayHandle) cancelAnimationFrame(graphRenderDelayHandle);
    graphRenderDelayHandle = requestAnimationFrame(() => {
      graphRenderDelayHandle = null;
      const latest = latestIncrementalSnapshot;
      if (
        !latest ||
        latest.documentKey !== documentKey ||
        latest.editorRevision !== revision ||
        latest.language !== language ||
        latest.sourceText !== renderText ||
        latest.isBlocked
      ) {
        if (pendingRenderSignature === renderSignature) pendingRenderSignature = '';
        return;
      }
      void deps.renderDocumentGraph({ kind: 'incremental', documentKey, language, text: renderText, revision }).then((result) => {
        const latestAfterAttach = latestIncrementalSnapshot;
        if (
          result == null &&
          latestAfterAttach?.documentKey === documentKey &&
          latestAfterAttach.editorRevision === revision &&
          latestAfterAttach.language === language &&
          latestAfterAttach.sourceText === renderText &&
          pendingRenderSignature === renderSignature
        ) pendingRenderSignature = '';
      });
    });
  }

  function maybeRenderJsonBlock(selection: JsonBlockSelection | null, hasRenderRuntime: boolean): void {
    if (!selection || !hasRenderRuntime) {
      if (lastJsonBlockActive) {
        deps.resetStreamProgress();
      }
      if (!selection) lastJsonBlockActive = false;
      pendingJsonBlockSignature = '';
      return;
    }
    lastJsonBlockActive = true;
    const signature = `${selection.blockDocumentKey}|${selection.revision}|${selection.startByte}|${selection.endByte}|${selection.text}`;
    if (pendingJsonBlockSignature === signature) return;
    pendingJsonBlockSignature = signature;
    deps.markGraphRequested({
      documentKey: selection.blockDocumentKey,
      revision: selection.revision,
      mode: 'json-block',
    });
    if (graphRenderDelayHandle) {
      cancelAnimationFrame(graphRenderDelayHandle);
      graphRenderDelayHandle = null;
    }
    void deps.renderJsonBlockSelection(selection).catch((error) => {
      if (pendingJsonBlockSignature === signature) pendingJsonBlockSignature = '';
      deps.onStreamingRenderError(error);
    });
  }

  function dispose(): void {
    if (graphRenderDelayHandle) cancelAnimationFrame(graphRenderDelayHandle);
    graphRenderDelayHandle = null;
    externalFullEditRenderAuthority.reset();
  }

  return { markRendered, maybeAttachFullEditSession, maybeRenderIncremental, maybeRenderJsonBlock, dispose };
}
