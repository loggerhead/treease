<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { get } from 'svelte/store';
  import type * as Monaco from 'monaco-editor';
  import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog';
  import { settingsDocument, settingsStore } from '../settings/settings-store';
  import { defaultSettings, settingsJsonSchema } from '../settings/ui-settings';
  import { initMonacoRuntime } from '../monaco/editor-runtime';
  import { loadMonacoJsonDefaults } from '../monaco/runtime-adapter';
  import { attachMonacoTestHook } from '../monaco/test-hook';
  import type { MonacoApi, MonacoDisposable, MonacoEditor, MonacoModel } from '../monaco/public-types';

  const settingsModelUri = 'inmemory://settings/dialog.json';
  const settingsSchemaUri = 'inmemory://settings/schema.json';

  export let open = false;

  let draft = '';
  let error = '';
  let dirty = false;
  let problemsCount = 0;
  let firstProblem = '';

  let monaco: MonacoApi | null = null;
  let editor: MonacoEditor | null = null;
  let model: MonacoModel | null = null;
  let editorContainer: HTMLDivElement | null = null;
  let markerListener: MonacoDisposable | null = null;
  let cleanupSettingsEditorTestHook: (() => void) | null = null;
  let suppressModelChange = false;
  let schemaConfigured = false;

  const groups = ['editor', 'formatting', 'viewer', 'interaction', 'parser'];

  function applyDraft(value: string) {
    draft = value;
    if (!model || model.getValue() === value) return;
    suppressModelChange = true;
    model.setValue(value);
    suppressModelChange = false;
  }

  function stringifySettingsDocument(value: unknown) {
    const nextValue = value === undefined ? defaultSettings : value;
    return JSON.stringify(nextValue, null, 2) ?? JSON.stringify(defaultSettings, null, 2);
  }

  function refreshProblems() {
    if (!monaco || !model) {
      problemsCount = 0;
      firstProblem = '';
      return;
    }
    const markers = monaco.editor
      .getModelMarkers({ resource: model.uri })
      .filter((marker) => marker.severity >= monaco!.MarkerSeverity.Warning);
    problemsCount = markers.length;
    firstProblem = markers[0]?.message ?? '';
  }

  async function configureSettingsSchema() {
    if (!monaco || schemaConfigured) return;
    const jsonDefaults = await loadMonacoJsonDefaults();
    jsonDefaults.setDiagnosticsOptions({
      validate: true,
      allowComments: false,
      enableSchemaRequest: false,
      schemaValidation: 'warning',
      schemas: [
        {
          uri: settingsSchemaUri,
          fileMatch: [settingsModelUri],
          schema: settingsJsonSchema
        }
      ]
    });
    schemaConfigured = true;
  }

  async function ensureMonacoEditor() {
    if (!open || !editorContainer || editor) return;
    if (!monaco) {
      const runtime = await initMonacoRuntime({
        callWasmWorker: (async () => new ArrayBuffer(0)) as <T>(
          type: string,
          payload?: Record<string, any>,
          transfer?: Transferable[]
        ) => Promise<T>,
        getTokenTypes: async () => []
      });
      monaco = runtime.monaco;
      runtime.ensureDocumentColorProvider('json');
    }
    await configureSettingsSchema();
    const uri = monaco.Uri.parse(settingsModelUri);
    model = monaco.editor.createModel(draft, 'json', uri);
    editor = monaco.editor.create(editorContainer, {
      model,
      minimap: { enabled: false },
      automaticLayout: true,
      scrollbar: { alwaysConsumeMouseWheel: false },
      overviewRulerBorder: true,
      colorDecorators: true,
      colorDecoratorsActivatedOn: 'clickAndHover',
      'semanticHighlighting.enabled': true,
    });
    cleanupSettingsEditorTestHook?.();
    cleanupSettingsEditorTestHook = attachMonacoTestHook(editor, 'settings-editor', monaco.editor.tokenize);
    markerListener?.dispose();
    markerListener = monaco.editor.onDidChangeMarkers((resources) => {
      if (!model) return;
      if (resources.some((resource) => resource.toString() === model?.uri.toString())) {
        refreshProblems();
      }
    });
    editor.onDidChangeModelContent(() => {
      if (!model || suppressModelChange) return;
      draft = model.getValue();
      dirty = true;
      error = '';
    });
    refreshProblems();
  }

  function disposeMonacoEditor() {
    cleanupSettingsEditorTestHook?.();
    cleanupSettingsEditorTestHook = null;
    markerListener?.dispose();
    markerListener = null;
    editor?.dispose();
    editor = null;
    model?.dispose();
    model = null;
    suppressModelChange = false;
    problemsCount = 0;
    firstProblem = '';
  }

  async function handleSave() {
    try {
      const parsed = JSON.parse(draft);
      await settingsStore.saveDocument(parsed);
      dirty = false;
      disposeMonacoEditor();
      open = false;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Invalid JSON';
    }
  }

  async function handleReset() {
    await settingsStore.reset();
    dirty = false;
    disposeMonacoEditor();
    open = false;
  }

  function handleOpenChange(next: boolean) {
    open = next;
    if (!next) {
      dirty = false;
      error = '';
      disposeMonacoEditor();
      return;
    }
    dirty = false;
    applyDraft(stringifySettingsDocument(get(settingsDocument)));
    error = '';
    void tick().then(() => ensureMonacoEditor());
  }

  $: if (open && !dirty) {
    applyDraft(stringifySettingsDocument($settingsDocument));
    error = '';
  }

  $: if (open && editorContainer && !editor) {
    void tick().then(() => ensureMonacoEditor());
  }

  onDestroy(() => {
    disposeMonacoEditor();
  });
</script>

<Dialog bind:open onOpenChange={handleOpenChange}>
<DialogContent aria-label="Settings dialog" data-testid="settings-dialog">
  <div class="flex w-full flex-col gap-4">
      <DialogHeader>
        <DialogTitle>Settings</DialogTitle>
      </DialogHeader>
    <div class="flex flex-1 flex-col gap-3">
      <div class="flex flex-wrap gap-2 rounded-[10px] border border-[var(--border-muted)] bg-[#f8fafc] p-1">
          {#each groups as group}
          <div class="rounded-full border border-[var(--border-muted)] bg-[var(--panel-bg)] px-2 py-1 text-[12px] text-[var(--text-muted)]">
            {group}
          </div>
          {/each}
          {#if problemsCount > 0}
          <div
            class="rounded-full border border-[var(--danger)] bg-[var(--panel-bg)] px-2 py-1 text-[12px] text-[var(--danger)]"
            data-testid="settings-problems-indicator"
          >
            {problemsCount} problem{problemsCount === 1 ? '' : 's'}
          </div>
          {/if}
        </div>
      <div
        class="min-h-[480px] w-full flex-1 overflow-hidden rounded-[10px] border border-[var(--border-muted)] bg-[var(--panel-bg)]"
        data-testid="monaco-settings-editor"
        data-monaco-test-hook="settings-editor"
        bind:this={editorContainer}
      ></div>
        {#if problemsCount > 0}
        <div
          class="rounded-[8px] border border-[var(--danger)]/20 bg-[var(--panel-bg)] px-3 py-2 text-[12px] text-[var(--text-muted)]"
          data-testid="settings-validation-summary"
        >
          <span class="text-[var(--danger)]">{problemsCount} problem{problemsCount === 1 ? '' : 's'}</span>
          <span> · invalid entries stay saved but fall back to defaults when applied</span>
          {#if firstProblem}
          <div class="mt-1 text-[var(--danger)]">{firstProblem}</div>
          {/if}
        </div>
        {/if}
        {#if error}
        <div class="text-[12px] text-[var(--danger)]">{error}</div>
        {/if}
      </div>
      <DialogFooter>
      <button
        class="min-w-[72px] rounded-[8px] border border-[var(--border-muted)] bg-[var(--panel-bg)] px-[14px] py-[6px] text-[12px] text-[var(--text-primary)]"
        on:click={handleReset}
        aria-label="Reset settings"
        title="Reset settings"
      >
        Reset
      </button>
      <button
        class="min-w-[72px] rounded-[8px] border border-[#2563eb] bg-[#2563eb] px-[14px] py-[6px] text-[12px] text-white"
        on:click={handleSave}
        aria-label="Save settings"
        title="Save settings"
      >
        Save
      </button>
      </DialogFooter>
    </div>
  </DialogContent>
</Dialog>
