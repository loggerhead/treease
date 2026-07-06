import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { editorStore } from './editor-store-internal';
import {
  sourceText,
  documentKey,
  languageId,
  compareEditToken,
  editorRevision,
  graphAppliedRevision,
  editorIO,
  editorMutation,
  treeState,
  fullEditUiState,
  editorWorkspace,
  activeTempModel,
  jsonBlockSelection,
} from './editor-store';

describe('editor-store', () => {
  beforeEach(() => {
    editorStore.reset();
  });

  describe('initial state', () => {
    it('exposes the default editor state', () => {
      expect(get(sourceText)).toBe('');
      expect(get(languageId)).toBe('json');
      expect(get(editorRevision)).toBe(0);
      expect(get(editorIO)).toBeNull();
      expect(get(editorMutation)).toBeNull();
      expect(get(treeState)).toEqual({
        tree: null,
        value: null,
        revision: 0,
        source: 'editor',
      });
      expect(get(fullEditUiState)).toEqual({
        active: false,
        sessionId: null,
        ownerKey: null,
        documentKey: null,
        revision: 0,
        streamSeq: 0,
        inputByteLength: 0,
        modelVersionId: null,
        byteLength: 0,
        language: '',
        phase: 'idle',
        sessionKind: null,
        transportKind: null,
        reason: null,
      });
      expect(editorStore.get().workspace.primaryTabId).toBe('primary');
      expect(editorStore.get().workspace.tabsById.primary).toMatchObject({
        role: 'primary',
        sourceText: '',
        documentKey: '',
        languageId: 'json',
        revision: 0,
      });
      expect('set' in editorWorkspace).toBe(false);
      expect('update' in editorWorkspace).toBe(false);

      const exposed = get(editorWorkspace) as any;
      const primaryId = exposed.primaryTabId;
      try {
        exposed.tabsById[primaryId].sourceText = 'mutated';
      } catch {}
      try {
        exposed.tabsById[primaryId].tempModel.treePath.push({ key: 'x' });
      } catch {}
      expect(editorStore.get().workspace.tabsById[primaryId].sourceText).not.toBe('mutated');
      expect(editorStore.get().workspace.tabsById[primaryId].tempModel.treePath).toEqual([]);

      const exposedState = editorStore.get() as any;
      const getPrimaryId = exposedState.workspace.primaryTabId;
      try {
        exposedState.workspace.tabsById[getPrimaryId].sourceText = 'mutated-through-get';
      } catch {}
      try {
        exposedState.workspace.tabsById[getPrimaryId].tempModel.treePath.push({ key: 'y' });
      } catch {}
      expect(editorStore.get().workspace.tabsById[getPrimaryId].sourceText).not.toBe('mutated-through-get');
      expect(editorStore.get().workspace.tabsById[getPrimaryId].tempModel.treePath).toEqual([]);

      const topLevelExposedState = editorStore.get() as any;
      try {
        topLevelExposedState.tempModel.treePath.push({ key: 'z' });
      } catch {}
      try {
        topLevelExposedState.fullEditUiState.phase = 'streaming';
      } catch {}
      expect(editorStore.get().workspace.tabsById[getPrimaryId].tempModel.treePath).toEqual([]);
      expect(editorStore.get().workspace.tabsById[getPrimaryId].fullEditUiState.phase).toBe('idle');

      const subscribedState = get(editorStore) as any;
      const subscribedPrimaryId = subscribedState.workspace.primaryTabId;
      try {
        subscribedState.workspace.tabsById[subscribedPrimaryId].sourceText = 'bad';
      } catch {}
      expect(editorStore.get().workspace.tabsById[subscribedPrimaryId].sourceText).not.toBe('bad');

      const exposedFullEditUiState = get(fullEditUiState) as any;
      try {
        exposedFullEditUiState.phase = 'streaming';
      } catch {}
      expect(editorStore.get().workspace.tabsById[subscribedPrimaryId].fullEditUiState.phase).toBe('idle');

      const exposedTempModel = get(activeTempModel) as any;
      try {
        exposedTempModel.treePath.push({ key: 'store-field' });
      } catch {}
      expect(editorStore.get().workspace.tabsById[subscribedPrimaryId].tempModel.treePath).toEqual([]);

      editorStore.actions.setTreeState({
        tree: { kind: 0, semType: 0, tag: '', value: '', children: [] } as any,
        value: { enabled: true },
        revision: 3,
        source: 'graph',
      });
      const exposedTreeStateFromStore = editorStore.get() as any;
      try {
        exposedTreeStateFromStore.treeState.tree.children.push({ kind: 1 });
      } catch {}
      try {
        exposedTreeStateFromStore.treeState.value.enabled = false;
      } catch {}
      expect((editorStore.get().treeState.tree as any)?.children).toEqual([]);
      expect((editorStore.get().treeState.value as any)?.enabled).toBe(true);

      const exposedTreeStateField = get(treeState) as any;
      try {
        exposedTreeStateField.tree.children.push({ kind: 2 });
      } catch {}
      try {
        exposedTreeStateField.value.enabled = false;
      } catch {}
      expect((get(treeState).tree as any)?.children).toEqual([]);
      expect((get(treeState).value as any)?.enabled).toBe(true);

      editorStore.actions.setJsonBlockSelection({
        sourceDocumentKey: 'source-doc',
        blockDocumentKey: 'block-doc',
        revision: 1,
        language: 'json',
        text: '{"x":1}',
        startByte: 0,
        endByte: 7,
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: 1,
        endColumn: 8,
      });
      const exposedJsonBlockSelection = get(jsonBlockSelection) as any;
      try {
        exposedJsonBlockSelection.text = '{"bad":true}';
      } catch {}
      expect(get(jsonBlockSelection)?.text).toBe('{"x":1}');

      editorStore.actions.emitMutation({
        type: 'replaceSourceText',
        payload: { text: '{"root":true}' },
      });
      const exposedEditorMutation = get(editorMutation) as any;
      try {
        exposedEditorMutation.mutation.payload.text = '{"bad":true}';
      } catch {}
      expect((get(editorMutation) as any)?.mutation.payload.text).toBe('{"root":true}');
    });
  });

  describe('actions', () => {
    it('updates scalar editor fields', () => {
      editorStore.actions.setSourceText('hello');
      expect(get(sourceText)).toBe('hello');
      editorStore.actions.setDocumentKey('abc123');
      expect(get(documentKey)).toBe('abc123');
      editorStore.actions.setLanguageId('yaml');
      expect(get(languageId)).toBe('yaml');
      const before = get(compareEditToken);
      editorStore.actions.incrementCompareEditToken();
      expect(get(compareEditToken)).toBe(before + 1);
      editorStore.actions.incrementEditorRevision();
      expect(get(editorRevision)).toBe(1);
      editorStore.actions.incrementEditorRevision();
      expect(get(editorRevision)).toBe(2);
      editorStore.actions.setGraphAppliedRevision(5);
      expect(get(graphAppliedRevision)).toBe(5);
    });

    it('mirrors direct editorRevision.update writes into the primary workspace tab', () => {
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });

      editorRevision.update((value) => value + 1);

      const state = editorStore.get();
      expect(get(editorRevision)).toBe(1);
      expect(state.workspace.tabsById[state.workspace.primaryTabId].revision).toBe(1);
    });

    it('mirrors direct editorRevision.set writes into the primary workspace tab', () => {
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });

      editorRevision.set(12);

      const state = editorStore.get();
      expect(get(editorRevision)).toBe(12);
      expect(state.workspace.tabsById[state.workspace.primaryTabId].revision).toBe(12);
    });

    it('emitMutation sets mutation with auto-incremented id', () => {
      editorStore.actions.emitMutation({
        type: 'replaceSourceText',
        payload: { text: '{"name":"Bob"}' },
      });
      const m = get(editorMutation);
      expect(m).not.toBeNull();
      expect(m!.id).toBeGreaterThan(0);
      expect(m!.mutation.type).toBe('replaceSourceText');
    });

    it('clearMutation sets mutation to null', () => {
      editorStore.actions.emitMutation({
        type: 'replaceSourceText',
        payload: { text: '{"name":"Bob"}' },
      });
      editorStore.actions.clearMutation();
      expect(get(editorMutation)).toBeNull();
    });

    it('emitMutation supports replaceSourceText', () => {
      editorStore.actions.emitMutation({
        type: 'replaceSourceText',
        payload: {
          text: '{"name":"Bob"}',
          graphEditFallback: {
            reason: 'unsupportedEdit',
            path: [{ key: 'name' } as any],
            kind: 'value',
          },
        },
      });
      const m = get(editorMutation);
      expect(m).not.toBeNull();
      expect(m!.mutation).toEqual({
        type: 'replaceSourceText',
        payload: {
          text: '{"name":"Bob"}',
          graphEditFallback: {
            reason: 'unsupportedEdit',
            path: [{ key: 'name' }],
            kind: 'value',
          },
        },
      });
    });

    it('setTreeState updates tree snapshot', () => {
      const nextTreeState = {
        tree: { kind: 0, semType: 0, tag: '', value: '', children: [] } as any,
        value: { enabled: true },
        revision: 3,
        source: 'graph' as const,
      };
      editorStore.actions.setTreeState(nextTreeState);
      expect(get(treeState)).toEqual(nextTreeState);
    });

    it('keeps treeState reads stable across unrelated updates', () => {
      const nextTreeState = {
        tree: { kind: 0, semType: 0, tag: '', value: '', children: [] } as any,
        value: { enabled: true },
        revision: 3,
        source: 'graph' as const,
      };
      editorStore.actions.setTreeState(nextTreeState);

      const events: any[] = [];
      const unsubscribe = treeState.subscribe((value) => {
        events.push(value);
      });
      const initialTreeRef = get(treeState).tree;
      const initialValueRef = get(treeState).value;

      editorStore.actions.setSourceText('hello');
      editorStore.actions.updateTempModel({ status: 'Loading' });

      expect(events).toHaveLength(1);
      expect(get(treeState).tree).toBe(initialTreeRef);
      expect(get(treeState).value).toBe(initialValueRef);
      unsubscribe();
    });

    it('isolates setTreeState input from later caller mutations', () => {
      const inputTreeState = {
        tree: { kind: 0, semType: 0, tag: '', value: '', children: [] } as any,
        value: { enabled: true },
        revision: 3,
        source: 'graph' as const,
      };
      editorStore.actions.setTreeState(inputTreeState);

      inputTreeState.tree.children.push({ kind: 1 } as any);
      (inputTreeState.value as any).enabled = false;

      expect((get(treeState).tree as any)?.children).toEqual([]);
      expect((get(treeState).value as any)?.enabled).toBe(true);
    });

    it('allows treeState.update callbacks to mutate a mutable clone', () => {
      treeState.set({
        tree: { kind: 0, semType: 0, tag: '', value: '', children: [] } as any,
        value: { enabled: true },
        revision: 3,
        source: 'graph',
      });

      expect(() => {
        treeState.update((state) => {
          (state.tree as any).children.push({ kind: 1 });
          (state.value as any).enabled = false;
          return state;
        });
      }).not.toThrow();

      expect((get(treeState).tree as any)?.children).toEqual([{ kind: 1 }]);
      expect((get(treeState).value as any)?.enabled).toBe(false);

      const exposed = get(treeState) as any;
      try {
        exposed.tree.children.push({ kind: 2 });
      } catch {}
      try {
        exposed.value.enabled = true;
      } catch {}

      expect((get(treeState).tree as any)?.children).toEqual([{ kind: 1 }]);
      expect((get(treeState).value as any)?.enabled).toBe(false);
    });

    it('updateTempModel merges partial', () => {
      editorStore.actions.updateTempModel({ status: 'Loading' });
      const state = editorStore.get();
      expect(state.tempModel.status).toBe('Loading');
      expect(state.tempModel.error).toBe('');  // other fields unchanged
    });

    it('isolates emitMutation input from later caller mutations', () => {
      const inputMutation = {
        type: 'replaceSourceText' as const,
        payload: {
          text: '{"root":true}',
          graphEditFallback: {
            reason: 'unsupportedEdit' as const,
            path: [{ key: 'root' } as any],
            kind: 'value' as const,
          },
        },
      };
      editorStore.actions.emitMutation(inputMutation);

      inputMutation.payload.text = '{"mutated":true}';
      inputMutation.payload.graphEditFallback.path.push({ key: 'mutated' } as any);

      expect((get(editorMutation) as any)?.mutation.payload.text).toBe('{"root":true}');
      expect((get(editorMutation) as any)?.mutation.payload.graphEditFallback.path).toEqual([{ key: 'root' }]);
    });

    it('isolates setTempModel input from later caller mutations', () => {
      const model = {
        ...editorStore.get().tempModel,
        treePath: [{ key: 'root' } as any],
        graphHighlight: {
          path: [{ key: 'root' } as any],
          revision: 1,
          source: 'editor' as const,
        },
      };
      editorStore.actions.setTempModel(model);

      model.treePath.push({ key: 'mutated' } as any);
      model.graphHighlight?.path.push({ key: 'mutated' } as any);

      expect(get(activeTempModel).treePath).toEqual([{ key: 'root' }]);
      expect(get(activeTempModel).graphHighlight?.path).toEqual([{ key: 'root' }]);
    });

    it('isolates updateTempModel input from later caller mutations', () => {
      const partial = {
        treePath: [{ key: 'root' } as any],
        graphHighlight: {
          path: [{ key: 'root' } as any],
          revision: 1,
          source: 'editor' as const,
        },
      };
      editorStore.actions.updateTempModel(partial);

      partial.treePath.push({ key: 'mutated' } as any);
      partial.graphHighlight.path.push({ key: 'mutated' } as any);

      expect(get(activeTempModel).treePath).toEqual([{ key: 'root' }]);
      expect(get(activeTempModel).graphHighlight?.path).toEqual([{ key: 'root' }]);
    });

    it('allows activeTempModel.update callbacks to mutate a mutable clone', () => {
      expect(() => {
        activeTempModel.update((model) => {
          model.treePath.push({ key: 'root' } as any);
          return model;
        });
      }).not.toThrow();

      expect(get(activeTempModel).treePath).toEqual([{ key: 'root' }]);
    });

    it('keeps activeTempModel reads stable across unrelated updates', () => {
      editorStore.actions.updateTempModel({
        treePath: [{ key: 'root' } as any],
      });

      const events: any[] = [];
      const unsubscribe = activeTempModel.subscribe((value) => {
        events.push(value);
      });
      const initialRef = get(activeTempModel);

      editorStore.actions.setSourceText('hello');
      editorStore.actions.incrementCompareEditToken();

      expect(events).toHaveLength(1);
      expect(get(activeTempModel)).toBe(initialRef);
      unsubscribe();
    });

    it('keeps activeTempModel stable when workspace reads include multiple tab temp models', () => {
      activeTempModel.update((model) => {
        model.treePath.push({ key: 'root' } as any);
        return model;
      });
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.ensureSidecarWorkspaceTab({
        id: 'tab-sidecar',
        name: 'Right Editor',
        sourceText: 'a: 1\n',
      });

      const before = get(activeTempModel);
      try {
        before.treePath.push({ key: 'mutate-attempt' } as any);
      } catch {}

      get(editorWorkspace);
      void editorStore.get().workspace;

      expect(get(activeTempModel)).toBe(before);
      expect(get(activeTempModel).treePath).toEqual([{ key: 'root' }]);
    });

    it('initializes workspace from the current primary editor state', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.incrementEditorRevision();

      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });

      const workspace = editorStore.get().workspace;
      expect(workspace.primaryTabId).toBe('tab-primary');
      expect(workspace.tabsById['tab-primary']).toMatchObject({
        role: 'primary',
        sourceText: '{"primary":true}',
        documentKey: 'tab-primary:0',
        languageId: 'json',
        revision: 1,
      });
    });

    it('adds a background workspace tab without changing primary compatibility fields', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });

      editorStore.actions.addWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
        revision: 0,
        graphAppliedRevision: 0,
        snapshotId: null,
        tempModel: editorStore.get().tempModel,
        fullEditUiState: editorStore.get().fullEditUiState,
      });

      const state = editorStore.get();
      expect(state.sourceText).toBe('{"primary":true}');
      expect(state.documentKey).toBe('tab-primary:0');
      expect(state.languageId).toBe('json');
      expect(state.workspace.tabOrder).toEqual(['tab-primary', 'tab-second']);
      expect(state.workspace.tabsById['tab-second']).toMatchObject({
        role: 'background',
        languageId: 'yaml',
        sourceText: 'name: second\n',
      });
    });

    it('activates a workspace tab and mirrors it into legacy primary fields', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.addWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
        revision: 4,
        graphAppliedRevision: 3,
        snapshotId: 12,
        tempModel: {
          ...editorStore.get().tempModel,
          scratchText: 'name: second\n',
          status: 'Ready',
        },
        fullEditUiState: editorStore.get().fullEditUiState,
      });

      editorStore.actions.activateWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
        revision: 4,
        graphAppliedRevision: 3,
        snapshotId: 12,
        tempModel: {
          ...editorStore.get().tempModel,
          scratchText: 'name: second\n',
          cursor: 'Ln 2, Col 1',
        },
        fullEditUiState: editorStore.get().fullEditUiState,
      });

      const state = editorStore.get();
      expect(state.sourceText).toBe('name: second\n');
      expect(state.documentKey).toBe('tab-second:0');
      expect(state.languageId).toBe('yaml');
      expect(state.editorRevision).toBe(4);
      expect(state.graphAppliedRevision).toBe(3);
      expect(state.tempModel.cursor).toBe('Ln 2, Col 1');
      expect(state.workspace.primaryTabId).toBe('tab-second');
      expect(state.workspace.tabsById['tab-primary'].role).toBe('background');
      expect(state.workspace.tabsById['tab-second'].role).toBe('primary');
    });

    it('activates an existing workspace tab with omitted optional fields without losing resolved state', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.addWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
        revision: 7,
        graphAppliedRevision: 6,
        snapshotId: 42,
        tempModel: {
          ...editorStore.get().tempModel,
          scratchText: 'name: second\n',
          cursor: 'Ln 4, Col 2',
          status: 'Second Ready',
        },
        fullEditUiState: {
          ...editorStore.get().fullEditUiState,
          documentKey: 'tab-second:0',
          revision: 7,
          byteLength: 13,
        },
      });

      const existingSecond = editorStore.get().workspace.tabsById['tab-second'];
      editorStore.actions.activateWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
      });

      const state = editorStore.get();
      expect(state.sourceText).toBe('name: second\n');
      expect(state.documentKey).toBe('tab-second:0');
      expect(state.languageId).toBe('yaml');
      expect(state.editorRevision).toBe(7);
      expect(state.graphAppliedRevision).toBe(6);
      expect(state.tempModel).toEqual(existingSecond.tempModel);
      expect(state.fullEditUiState).toEqual(existingSecond.fullEditUiState);
      expect(state.workspace.tabsById['tab-second'].role).toBe('primary');
    });

    it('closes an inactive workspace tab without changing active primary fields', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.incrementEditorRevision();
      editorStore.actions.setGraphAppliedRevision(1);
      editorStore.actions.updateTempModel({
        cursor: 'Ln 3, Col 7',
        scratchText: '{"primary":true}',
      });
      editorStore.actions.setFullEditUiState({
        ...editorStore.get().fullEditUiState,
        documentKey: 'tab-primary:0',
        revision: 1,
        byteLength: 16,
      });
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.addWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
        revision: 0,
        graphAppliedRevision: 0,
        snapshotId: null,
        tempModel: editorStore.get().tempModel,
        fullEditUiState: editorStore.get().fullEditUiState,
      });

      const before = editorStore.get();
      editorStore.actions.closeWorkspaceTabFromEditor('tab-second');

      const state = editorStore.get();
      expect(state.sourceText).toBe(before.sourceText);
      expect(state.previousSourceText).toBe(before.previousSourceText);
      expect(state.documentKey).toBe(before.documentKey);
      expect(state.languageId).toBe(before.languageId);
      expect(state.editorRevision).toBe(before.editorRevision);
      expect(state.graphAppliedRevision).toBe(before.graphAppliedRevision);
      expect(state.tempModel).toEqual(before.tempModel);
      expect(state.fullEditUiState).toEqual(before.fullEditUiState);
      expect(state.workspace.tabOrder).toEqual(['tab-primary']);
      expect(state.workspace.tabsById['tab-second']).toBeUndefined();
      expect(state.workspace.primaryTabId).toBe('tab-primary');
    });

    it('closes the active workspace tab and mirrors the promoted previous tab into legacy fields', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.incrementEditorRevision();
      editorStore.actions.setGraphAppliedRevision(1);
      editorStore.actions.updateTempModel({
        scratchText: '{"primary":true}',
        cursor: 'Ln 1, Col 17',
      });
      editorStore.actions.setFullEditUiState({
        ...editorStore.get().fullEditUiState,
        documentKey: 'tab-primary:0',
        revision: 1,
        byteLength: 16,
      });
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.addWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
        revision: 4,
        graphAppliedRevision: 3,
        snapshotId: 12,
        tempModel: {
          ...editorStore.get().tempModel,
          scratchText: 'name: second\n',
          cursor: 'Ln 2, Col 1',
        },
        fullEditUiState: {
          ...editorStore.get().fullEditUiState,
          documentKey: 'tab-second:0',
          revision: 4,
          byteLength: 13,
        },
      });
      editorStore.actions.activateWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
      });

      const promotedPrimary = editorStore.get().workspace.tabsById['tab-primary'];
      editorStore.actions.closeWorkspaceTabFromEditor('tab-second');

      const state = editorStore.get();
      expect(state.sourceText).toBe(promotedPrimary.sourceText);
      expect(state.documentKey).toBe(promotedPrimary.documentKey);
      expect(state.languageId).toBe(promotedPrimary.languageId);
      expect(state.editorRevision).toBe(promotedPrimary.revision);
      expect(state.graphAppliedRevision).toBe(promotedPrimary.graphAppliedRevision);
      expect(state.tempModel).toEqual(promotedPrimary.tempModel);
      expect(state.fullEditUiState).toEqual(promotedPrimary.fullEditUiState);
      expect(state.workspace.primaryTabId).toBe('tab-primary');
      expect(state.workspace.tabsById['tab-primary'].role).toBe('primary');
      expect(state.workspace.tabsById['tab-second']).toBeUndefined();
    });

    it('closes the last active workspace tab with a valid fallback and mirrors fallback fields', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });

      editorStore.actions.closeWorkspaceTabFromEditor('tab-primary', {
        id: 'tab-fallback',
        name: 'Untitled Fallback',
        documentKey: 'tab-fallback:0',
        languageId: 'toml' as any,
        sourceText: 'name = "fallback"\n',
        revision: 9,
        graphAppliedRevision: 8,
        snapshotId: 90,
        tempModel: {
          ...editorStore.get().tempModel,
          scratchText: 'name = "fallback"\n',
          cursor: 'Ln 1, Col 18',
        },
        fullEditUiState: {
          ...editorStore.get().fullEditUiState,
          documentKey: 'tab-fallback:0',
          revision: 9,
          byteLength: 18,
        },
      });

      const state = editorStore.get();
      const fallbackTab = state.workspace.tabsById['tab-fallback'];
      expect(state.sourceText).toBe(fallbackTab.sourceText);
      expect(state.documentKey).toBe(fallbackTab.documentKey);
      expect(state.languageId).toBe(fallbackTab.languageId);
      expect(state.editorRevision).toBe(fallbackTab.revision);
      expect(state.graphAppliedRevision).toBe(fallbackTab.graphAppliedRevision);
      expect(state.tempModel).toEqual(fallbackTab.tempModel);
      expect(state.fullEditUiState).toEqual(fallbackTab.fullEditUiState);
      expect(state.workspace.primaryTabId).toBe('tab-fallback');
      expect(state.workspace.tabOrder).toEqual(['tab-fallback']);
      expect(state.workspace.tabsById['tab-primary']).toBeUndefined();
    });

    it('returns workspace tab summaries for left tabs in order and excludes sidecar', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.addWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
      });
      editorStore.actions.ensureSidecarWorkspaceTab({
        id: 'tab-sidecar',
        name: 'Right Editor',
        sourceText: 'sidecar: true\n',
      });

      expect(editorStore.actions.getWorkspaceTabSummaries()).toEqual([
        { id: 'tab-primary', name: 'Untitled 1', languageId: 'json' },
        { id: 'tab-second', name: 'Untitled 2', languageId: 'yaml' },
      ]);
    });

    it('updates sidecar tab state without changing primary compatibility fields', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });

      editorStore.actions.ensureSidecarWorkspaceTab({
        id: 'tab-sidecar',
        name: 'Right Editor',
        sourceText: 'a: 1\n',
      });
      editorStore.actions.updateWorkspaceTab('tab-sidecar', {
        sourceText: 'a: 2\n',
        revision: 4,
        snapshotId: 44,
      });

      const state = editorStore.get();
      expect(state.sourceText).toBe('{"primary":true}');
      expect(state.documentKey).toBe('tab-primary:0');
      expect(state.editorRevision).toBe(0);
      expect(state.workspace.tabsById['tab-sidecar']).toMatchObject({
        role: 'sidecar',
        languageId: 'json',
        sourceText: 'a: 2\n',
        revision: 4,
        snapshotId: 44,
      });
    });

    it('allows explicit sidecar language patches while applying other generic tab updates', () => {
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.ensureSidecarWorkspaceTab({
        id: 'tab-sidecar',
        name: 'Right Editor',
        sourceText: 'a: 1\n',
      });

      editorStore.actions.updateWorkspaceTab('tab-sidecar', {
        languageId: 'yaml' as any,
        sourceText: 'changed',
      });

      expect(editorStore.get().workspace.tabsById['tab-sidecar']).toMatchObject({
        languageId: 'yaml',
        sourceText: 'changed',
      });
    });

    it('ignores background tab language patches to keep inactive editor metadata isolated', () => {
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.addWorkspaceTabFromEditor({
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
      });

      editorStore.actions.updateWorkspaceTab('tab-second', {
        languageId: 'toml' as any,
        sourceText: 'changed',
      });

      expect(editorStore.get().workspace.tabsById['tab-second']).toMatchObject({
        role: 'background',
        languageId: 'yaml',
        sourceText: 'changed',
      });
    });

    it('ignores direct workspace primary tab updates to protect legacy compatibility fields', () => {
      editorStore.actions.setSourceText('{"primary":true}');
      editorStore.actions.setDocumentKey('tab-primary:0');
      editorStore.actions.incrementEditorRevision();
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });

      const before = editorStore.get();
      editorStore.actions.updateWorkspaceTab(before.workspace.primaryTabId, {
        sourceText: 'bad',
        revision: 99,
      });

      const state = editorStore.get();
      expect(state.sourceText).toBe('{"primary":true}');
      expect(state.editorRevision).toBe(1);
      expect(state.workspace.tabsById[state.workspace.primaryTabId]).toMatchObject({
        sourceText: '{"primary":true}',
        revision: 1,
      });
    });

    it('syncs sidecar language when primary language changes', () => {
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.ensureSidecarWorkspaceTab({
        id: 'tab-sidecar',
        name: 'Right Editor',
        sourceText: '{}',
      });

      editorStore.actions.setLanguageId('toml');

      expect(editorStore.get().workspace.tabsById['tab-sidecar'].languageId).toBe('toml');
    });

    it('syncs sidecar language when the languageId field store is written directly', () => {
      editorStore.actions.setLanguageId('json');
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.actions.ensureSidecarWorkspaceTab({
        id: 'tab-sidecar',
        name: 'Right Editor',
        sourceText: '{}',
      });

      languageId.set('yaml');

      expect(editorStore.get().workspace.tabsById['tab-sidecar'].languageId).toBe('yaml');
    });

    it('resetTempModel restores defaults', () => {
      editorStore.actions.updateTempModel({ status: 'Error', error: 'fail' });
      editorStore.actions.resetTempModel();
      const state = editorStore.get();
      expect(state.tempModel.status).toBe('Ready');
      expect(state.tempModel.error).toBe('');
    });

    it('tracks full-edit stream phase transitions for the active session', () => {
      editorStore.actions.setGraphAppliedRevision(9);
      editorStore.actions.prepareFullEditStream({
        documentKey: 'doc-1',
        revision: 3,
        language: 'yaml',
        transportKind: 'memory',
        reason: 'language-example',
      });
      expect(get(graphAppliedRevision)).toBe(2);
      expect(get(fullEditUiState)).toMatchObject({
        active: true,
        sessionId: null,
        ownerKey: null,
        documentKey: 'doc-1',
        revision: 3,
        language: 'yaml',
        phase: 'preparing',
        sessionKind: 'full-edit',
        transportKind: 'memory',
        reason: 'language-example',
      });

      editorStore.actions.cancelPreparedFullEditStream({
        documentKey: 'other-doc',
        revision: 3,
        reason: 'language-example',
      });
      expect(get(fullEditUiState).phase).toBe('preparing');

      editorStore.actions.beginFullEditStream({
        sessionId: 'session-1',
        ownerKey: 'owner-1',
        documentKey: 'doc-1',
        revision: 3,
        language: 'yaml',
        transportKind: 'memory',
        reason: 'language-example',
      });
      expect(get(graphAppliedRevision)).toBe(2);
      expect(get(fullEditUiState)).toMatchObject({
        active: true,
        sessionId: 'session-1',
        ownerKey: 'owner-1',
        documentKey: 'doc-1',
        revision: 3,
        language: 'yaml',
        phase: 'streaming',
        sessionKind: 'full-edit',
        transportKind: 'memory',
        reason: 'language-example',
      });

      editorStore.actions.cancelPreparedFullEditStream({
        documentKey: 'doc-1',
        revision: 3,
        reason: 'language-example',
      });
      expect(get(fullEditUiState).phase).toBe('streaming');

      editorStore.actions.appendFullEditStreamChunkMeta({
        sessionId: 'session-1',
        ownerKey: 'owner-1',
        streamSeq: 2,
        inputByteLength: 32,
        modelVersionId: 7,
      });
      expect(get(fullEditUiState)).toMatchObject({
        streamSeq: 2,
        inputByteLength: 32,
        byteLength: 32,
        modelVersionId: 7,
      });

      editorStore.actions.markFullEditStreamFinalizing({ sessionId: 'session-1', ownerKey: 'owner-1' });
      expect(get(fullEditUiState).phase).toBe('finalizing');

      editorStore.actions.markFullEditStreamSettled({ sessionId: 'session-1', ownerKey: 'owner-1' });
      expect(get(fullEditUiState).phase).toBe('settled');

      editorStore.actions.completeFullEditStreamUi({ sessionId: 'session-1', ownerKey: 'owner-1' });
      expect(get(fullEditUiState)).toMatchObject({
        active: true,
        sessionId: 'session-1',
        ownerKey: 'owner-1',
        phase: 'idle',
      });

      editorStore.actions.finishFullEditStream({ sessionId: 'session-1', ownerKey: 'owner-1' });
      expect(get(fullEditUiState)).toEqual({
        active: false,
        sessionId: null,
        ownerKey: null,
        documentKey: null,
        revision: 0,
        streamSeq: 0,
        inputByteLength: 0,
        modelVersionId: null,
        byteLength: 0,
        language: '',
        phase: 'idle',
        sessionKind: null,
        transportKind: null,
        reason: null,
      });
    });

    it('syncs primary workspace tab full-edit state across stream lifecycle', () => {
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });

      editorStore.actions.beginFullEditStream({
        sessionId: 'session-4',
        ownerKey: 'owner-4',
        documentKey: 'doc-4',
        revision: 6,
        language: 'json',
        transportKind: 'memory',
        reason: 'whole-document-replacement',
      });
      expect(editorStore.get().workspace.tabsById['tab-primary'].fullEditUiState.phase).toBe('streaming');

      editorStore.actions.finishFullEditStream({ sessionId: 'session-4', ownerKey: 'owner-4' });
      expect(editorStore.get().workspace.tabsById['tab-primary'].fullEditUiState.phase).toBe('idle');
      expect(editorStore.get().workspace.tabsById['tab-primary'].fullEditUiState.active).toBe(false);
    });

    it('clears a matching prepared full-edit state before a session exists', () => {
      editorStore.actions.prepareFullEditStream({
        documentKey: 'doc-2',
        revision: 4,
        language: 'json',
        transportKind: 'memory',
        reason: 'whole-document-replacement',
      });

      editorStore.actions.cancelPreparedFullEditStream({
        documentKey: 'doc-2',
        revision: 4,
        reason: 'whole-document-replacement',
      });

      expect(get(fullEditUiState)).toMatchObject({
        active: false,
        phase: 'idle',
        sessionId: null,
      });
    });

    it('ignores stale or non-monotonic full-edit chunk metadata', () => {
      editorStore.actions.beginFullEditStream({
        sessionId: 'session-2',
        ownerKey: 'owner-2',
        documentKey: 'doc-2',
        revision: 4,
        language: 'json',
        transportKind: 'file',
        reason: 'import-file',
      });
      editorStore.actions.appendFullEditStreamChunkMeta({
        sessionId: 'session-2',
        ownerKey: 'owner-2',
        streamSeq: 3,
        inputByteLength: 128,
        modelVersionId: 9,
      });

      const stableState = get(fullEditUiState);

      editorStore.actions.appendFullEditStreamChunkMeta({
        sessionId: 'session-2',
        ownerKey: 'owner-2',
        streamSeq: 2,
        inputByteLength: 256,
        modelVersionId: 10,
      });
      expect(get(fullEditUiState)).toEqual(stableState);

      editorStore.actions.appendFullEditStreamChunkMeta({
        sessionId: 'session-2',
        ownerKey: 'owner-2',
        streamSeq: 4,
        inputByteLength: 64,
        modelVersionId: 10,
      });
      expect(get(fullEditUiState)).toEqual(stableState);

      editorStore.actions.appendFullEditStreamChunkMeta({
        sessionId: 'session-stale',
        ownerKey: 'owner-2',
        streamSeq: 4,
        inputByteLength: 256,
        modelVersionId: 10,
      });
      expect(get(fullEditUiState)).toEqual(stableState);
    });

    it('ignores full-edit phase transitions from stale sessions', () => {
      editorStore.actions.beginFullEditStream({
        sessionId: 'session-3',
        ownerKey: 'owner-3',
        documentKey: 'doc-3',
        revision: 5,
        language: 'json',
        transportKind: 'file',
        reason: 'drop-file',
      });

      editorStore.actions.markFullEditStreamFinalizing({ sessionId: 'session-stale', ownerKey: 'owner-3' });
      expect(get(fullEditUiState).phase).toBe('streaming');

      editorStore.actions.markFullEditStreamSettled({ sessionId: 'session-3', ownerKey: 'owner-3' });
      expect(get(fullEditUiState).phase).toBe('streaming');

      editorStore.actions.markFullEditStreamFinalizing({ sessionId: 'session-3', ownerKey: 'owner-3' });
      editorStore.actions.markFullEditStreamSettled({ sessionId: 'session-3', ownerKey: 'owner-3' });
      expect(get(fullEditUiState).phase).toBe('settled');

      editorStore.actions.cancelFullEditStream({ sessionId: 'session-stale', ownerKey: 'owner-3' });
      expect(get(fullEditUiState).phase).toBe('settled');

      editorStore.actions.cancelFullEditStream({ sessionId: 'session-3', ownerKey: 'owner-3' });
      expect(get(fullEditUiState).active).toBe(false);
      expect(get(fullEditUiState).phase).toBe('idle');
    });
  });
  describe('reset', () => {
    it('resets all state to initial values', () => {
      editorStore.actions.setSourceText('modified');
      editorStore.actions.setLanguageId('yaml');
      editorStore.actions.incrementEditorRevision();
      editorStore.actions.initWorkspaceFromPrimaryTab({
        id: 'tab-primary',
        name: 'Untitled 1',
      });
      editorStore.reset();
      expect(get(sourceText)).toBe('');
      expect(get(languageId)).toBe('json');
      expect(get(editorRevision)).toBe(0);
      expect(editorStore.get().workspace.primaryTabId).toBe('primary');
      expect(editorStore.get().workspace.tabsById.primary).toMatchObject({
        role: 'primary',
        sourceText: '',
        documentKey: '',
        languageId: 'json',
        revision: 0,
      });
    });
  });

  describe('get', () => {
    it('returns current snapshot', () => {
      editorStore.actions.setSourceText('snap');
      const state = editorStore.get();
      expect(state.sourceText).toBe('snap');
    });
  });
});
