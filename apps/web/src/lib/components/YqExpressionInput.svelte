<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import type * as Monaco from 'monaco-editor'
  import { X } from 'lucide-svelte'
  import { Button, IconButton } from './ui/button'
  import { initMonacoRuntime } from '../monaco/editor-runtime'
  import { hasYqCompletionMatches, YQ_LANGUAGE_ID } from '../monaco/yq-language-support'
  import { attachMonacoTestHook } from '../monaco/test-hook'
  import { documentKey } from '../store/editor-store'
  import { getActiveDocumentSnapshotId } from '../services/DocumentSessionService'
  import { queryFieldLabels } from '../services/SnapshotProjectionService'
  import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton'

  export let value = ''
  export let busy = false
  export let error = ''
  export let onChange: (value: string) => void = () => {}
  export let onSubmit: (value: string) => void | Promise<void> = () => {}
  export let onClose: () => void = () => {}

  let container: HTMLDivElement | null = null
  let editor: Monaco.editor.IStandaloneCodeEditor | null = null
  let model: Monaco.editor.ITextModel | null = null
  let cleanupYqEditorTestHook: (() => void) | null = null
  let yqPathCompletionDisposable: Monaco.IDisposable | null = null
  let editorReady: Promise<void> | null = null
  let syncingValue = false
  let suppressNextSuggestTrigger = false
  let suggestTriggerVersion = 0
  let cachedPathCompletionSignature = ''
  let cachedPathCompletionLabels: string[] = []
  let currentValue = normalizeExpressionInput(value)

  function suppressSuggestTriggerForCurrentTurn() {
    suppressNextSuggestTrigger = true
    queueMicrotask(() => {
      suppressNextSuggestTrigger = false
    })
  }

  export async function focus() {
    await ensureEditor()
    editor?.focus()
  }

  function normalizeExpressionInput(next: string) {
    return next.replace(/[\r\n]+/g, ' ')
  }

  function normalizeSubmittedExpression(next: string) {
    return normalizeExpressionInput(next).trim()
  }

  function getSuggestController() {
    return editor?.getContribution('editor.contrib.suggestController') as
      | {
          selectFirstSuggestion?: () => void
          acceptSelectedSuggestion?: (keepAlternativeSuggestions: boolean, alternativeOverwriteConfig?: boolean) => void
          cancelSuggestWidget?: () => void
        }
      | undefined
  }

  function getCurrentCompletionQuery() {
    if (!editor || !model) return ''
    const position = editor.getPosition()
    if (!position) return ''
    const lineContent = model.getLineContent(position.lineNumber)
    const beforeCursor = lineContent.slice(0, position.column - 1)
    const codecMatch = beforeCursor.match(/@[A-Za-z_]*$/)
    if (codecMatch) return codecMatch[0]
    const word = model.getWordUntilPosition(position)
    if (word.endColumn <= word.startColumn) return ''
    return lineContent.slice(word.startColumn - 1, position.column - 1)
  }

  function getCurrentPathCompletionPrefix() {
    if (!editor || !model) return null
    const position = editor.getPosition()
    if (!position) return null
    const beforeCursor = model.getLineContent(position.lineNumber).slice(0, position.column - 1)
    return beforeCursor.match(/\.([A-Za-z_][\w-]*)?$/)?.[1] ?? null
  }

  async function getYqPathCompletionLabels(prefix: string) {
    const normalizedPrefix = prefix.toLowerCase()
    const activeDocumentKey = $documentKey
    const snapshotId = getActiveDocumentSnapshotId(activeDocumentKey)
    const signature = activeDocumentKey && snapshotId != null ? `${activeDocumentKey}:${snapshotId}` : ''
    if (signature !== cachedPathCompletionSignature) {
      cachedPathCompletionSignature = signature
      cachedPathCompletionLabels = signature
        ? (await queryFieldLabels({ documentKey: activeDocumentKey, snapshotId }))
            .filter((label) => /^[A-Za-z_][\w-]*$/.test(label))
            .sort((left, right) => left.localeCompare(right))
        : []
    }
    return cachedPathCompletionLabels
      .filter((label) => !normalizedPrefix || label.toLowerCase().startsWith(normalizedPrefix))
  }

  function hasYqPathCompletionMatches(prefix: string | null) {
    return prefix !== null
  }

  function registerYqPathCompletionProvider(monaco: typeof Monaco) {
    yqPathCompletionDisposable?.dispose()
    yqPathCompletionDisposable = monaco.languages.registerCompletionItemProvider(YQ_LANGUAGE_ID, {
      triggerCharacters: ['.'],
      async provideCompletionItems(model, position) {
        const beforeCursor = model.getLineContent(position.lineNumber).slice(0, position.column - 1)
        const match = beforeCursor.match(/\.([A-Za-z_][\w-]*)?$/)
        if (!match) return { suggestions: [] }
        const prefix = match[1] ?? ''
        const range = {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: position.column - prefix.length,
          endColumn: position.column
        }
        return {
          suggestions: (await getYqPathCompletionLabels(prefix)).map((label) => ({
            label,
            kind: monaco.languages.CompletionItemKind.Field,
            insertText: label,
            detail: 'source field',
            range
          }))
        }
      }
    })
  }

  async function handleSubmit() {
    const next = normalizeSubmittedExpression(model?.getValue() ?? currentValue)
    onChange(next)
    await onSubmit(next)
  }

  async function ensureEditor() {
    if (editor) return
    if (editorReady) return editorReady
    editorReady = (async () => {
      if (!container || editor) return
      const runtime = await initMonacoRuntime({
        callWasmWorker: callSharedWasmWorker,
        getTokenTypes: () => callSharedWasmWorker<readonly string[]>('semanticTokensLegend')
      })
      if (!container || editor) return
      const monaco = runtime.monaco
      runtime.ensureYqLanguageSupport()
      registerYqPathCompletionProvider(monaco)
      const uri = monaco.Uri.parse(`inmemory://yq-expression/${Date.now()}`)
      model = monaco.editor.createModel(currentValue, YQ_LANGUAGE_ID, uri)
      editor = monaco.editor.create(container, {
        model,
        automaticLayout: true,
        minimap: { enabled: false },
        lineNumbers: 'off',
        glyphMargin: false,
        folding: false,
        lineDecorationsWidth: 0,
        scrollBeyondLastLine: false,
        wordWrap: 'off',
        quickSuggestions: false,
        suggestOnTriggerCharacters: true,
        tabCompletion: 'on',
        acceptSuggestionOnEnter: 'on',
        fixedOverflowWidgets: true,
        lineHeight: 20,
        fontSize: 13,
        scrollbar: {
          vertical: 'hidden',
          horizontal: 'hidden'
        },
        renderLineHighlight: 'none',
        placeholder: 'Enter a jq filter, for example .items[0] or .items[] | .name',
        padding: { top: 4, bottom: 4 }
      })
      cleanupYqEditorTestHook?.()
      cleanupYqEditorTestHook = attachMonacoTestHook(editor, 'yq-expression-input', monaco.editor.tokenize)
      editor.onDidChangeModelContent((event) => {
        if (!model || syncingValue) return
        const raw = model.getValue()
        const next = normalizeExpressionInput(raw)
        if (next !== raw) {
          syncingValue = true
          model.setValue(next)
          syncingValue = false
        }
        currentValue = next
        onChange(next)
        if (suppressNextSuggestTrigger) {
          return
        }
        if (!busy && event.changes.some((change) => /[A-Za-z_]/.test(change.text))) {
          const completionQuery = getCurrentCompletionQuery()
          const pathCompletionPrefix = getCurrentPathCompletionPrefix()
          const suggestController = getSuggestController()
          if (!hasYqCompletionMatches(completionQuery) && !hasYqPathCompletionMatches(pathCompletionPrefix)) {
            suggestTriggerVersion += 1
            suggestController?.cancelSuggestWidget?.()
            return
          }
          const scheduledTriggerVersion = ++suggestTriggerVersion
          queueMicrotask(() => {
            if (scheduledTriggerVersion !== suggestTriggerVersion) return
            editor?.trigger('yq-expression-input', 'editor.action.triggerSuggest', {})
          })
        }
      })
      editor.addCommand(monaco.KeyCode.Enter, () => {
        const suggestController = getSuggestController()
        if (!suggestController?.acceptSelectedSuggestion) return
        suggestTriggerVersion += 1
        suggestController.selectFirstSuggestion?.()
        suppressSuggestTriggerForCurrentTurn()
        suggestController.acceptSelectedSuggestion(false, false)
        suggestController.cancelSuggestWidget?.()
      }, 'suggestWidgetVisible')
      editor.addCommand(monaco.KeyCode.Enter, () => {
        suggestTriggerVersion += 1
        suppressSuggestTriggerForCurrentTurn()
        void handleSubmit()
      }, '!suggestWidgetVisible')
      editor.addCommand(monaco.KeyCode.Escape, () => {
        onClose()
      })
      editor.focus()
      const line = model.getLineCount()
      const column = model.getLineMaxColumn(line)
      editor.setPosition({ lineNumber: line, column })
    })().finally(() => {
      editorReady = null
    })
    return editorReady
  }

  $: {
    const next = normalizeExpressionInput(value)
    if (next !== currentValue) {
      currentValue = next
      if (model && model.getValue() !== next) {
        syncingValue = true
        model.setValue(next)
        syncingValue = false
      }
    }
  }

  $: editor?.updateOptions({ readOnly: busy })

  onMount(() => {
    void ensureEditor()
  })

  onDestroy(() => {
    cleanupYqEditorTestHook?.()
    cleanupYqEditorTestHook = null
    yqPathCompletionDisposable?.dispose()
    yqPathCompletionDisposable = null
    editor?.dispose()
    model?.dispose()
  })
</script>

<div class="border-t border-[var(--border-strong)] bg-[var(--panel-bg-alt)] px-2 py-1.5" data-testid="yq-expression-panel">
  {#if error}
    <p class="mb-1 text-[12px] text-[var(--danger-text,#dc2626)]" data-testid="yq-expression-error">{error}</p>
  {/if}
  <div class="flex items-center gap-1.5">
    <div class="min-w-0 flex-1 overflow-visible rounded-[8px] border border-[var(--border-muted)] bg-[var(--panel-bg)]">
      <div bind:this={container} class="h-[30px] w-full" data-testid="yq-expression-editor"></div>
    </div>
    <Button size="xs" on:click={() => void handleSubmit()} disabled={busy || !currentValue.trim()}>
      {#if busy}Running{:else}Run{/if}
    </Button>
    <IconButton aria-label="Close yq input" title="Close" on:click={onClose} disabled={busy}>
      <X size={12} />
    </IconButton>
  </div>
</div>
