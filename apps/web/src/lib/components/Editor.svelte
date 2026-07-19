<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { PathSeg } from '../store/tree-path';
  import type { DiffPlan } from '../graph/diff-plan';
  import type { SupportedEditorLanguageId } from '../monaco/language-support';
  import type { RuntimeStateEventDetail } from '../runtime-loading';
  import EditorCore from './Editor/EditorCore.svelte';
  import type { TabSummary } from './Editor/types';

  export let tabSummaries: TabSummary[] = [];
  export let activeTabId = '';
  export let onScroll: (payload: { scrollTop: number; scrollLeft: number }) => void = () => {};
  export let enableRevealSync = true;
  export let synchronizedRuntimeLoading = false;

  const dispatch = createEventDispatcher<{ reveal: unknown; 'runtime-state': RuntimeStateEventDetail }>();

  let editorCore: EditorCore;

  export function addTab() {
    editorCore?.addTab();
  }

  export async function ensureReady() {
    await editorCore?.ensureReady?.();
  }

  export async function waitForIdle() {
    await editorCore?.waitForIdle?.();
  }

  export function closeTab(id: string) {
    editorCore?.closeTab(id);
  }

  export function activateTab(id: string) {
    editorCore?.activateTab(id);
  }

  export function formatActive() {
    return editorCore?.formatActive();
  }

  export function minifyActive() {
    return editorCore?.minifyActive();
  }

  export function compactActive() {
    return editorCore?.compactActive();
  }

  export function sortActive() {
    return editorCore?.sortActive();
  }

  export function exportAs(targetFormat: string) {
    return editorCore?.exportAs(targetFormat);
  }

  export function getActiveText() {
    return editorCore?.getActiveText();
  }

  export function getActiveLanguage() {
    return editorCore?.getActiveLanguage();
  }

  export function importAs(targetFormat: string, text: string, sourceFormat: string) {
    return editorCore?.importAs(targetFormat, text, sourceFormat);
  }

  export function openDocument(payload: {
    name: string;
    text: string;
    languageId: SupportedEditorLanguageId;
    fileLinkedDocument?: { grantId: string; name: string };
  }) {
    return editorCore?.openDocument(payload);
  }

  export function replaceActiveFromFile(payload: { text: string; languageId: SupportedEditorLanguageId }) {
    return editorCore?.replaceActiveFromFile(payload);
  }

  export function replaceDocumentFromFile(payload: { tabId: string; text: string; languageId: SupportedEditorLanguageId }) {
    return editorCore?.replaceDocumentFromFile(payload);
  }

  export function renameDocument(tabId: string, name: string) {
    return editorCore?.renameDocument(tabId, name);
  }

  export function importStream(
    file: File,
    sourceLanguage: string,
    targetLanguage: SupportedEditorLanguageId | undefined,
  ) {
    return editorCore?.importStream(file, sourceLanguage, targetLanguage);
  }

  export function revealError(startLineNumber: number, startColumn: number) {
    editorCore?.revealError(startLineNumber, startColumn);
  }

  export function revealLine(lineNumber: number, column = 1) {
    editorCore?.revealLine(lineNumber, column);
  }

  export function revealPath(path: PathSeg[], options: { target?: 'key' | 'value' | 'node' } | undefined) {
    return editorCore?.revealPath(path, options);
  }

  export function getScrollPosition() {
    return editorCore?.getScrollPosition();
  }

  export function setScrollPosition(position: { scrollTop: number; scrollLeft: number }) {
    editorCore?.setScrollPosition(position);
  }

  export function getViewportAnchor() {
    return editorCore?.getViewportAnchor();
  }

  export function restoreViewportAnchor(anchor: { topLine: number; scrollLeft: number }) {
    editorCore?.restoreViewportAnchor(anchor);
  }

  export function getSelection() {
    return editorCore?.getSelection() ?? null;
  }

  export function restoreSelection(selection: { startLine: number; startColumn: number; endLine: number; endColumn: number }) {
    editorCore?.restoreSelection(selection);
  }

  export function applyDiffPlan(plan: DiffPlan) {
    editorCore?.applyDiffPlan(plan);
  }
  export async function escapeActive(): Promise<void> {
    return editorCore?.escapeActive();
  }

  export async function unescapeActive(): Promise<void> {
    return editorCore?.unescapeActive();
  }

  function handleReveal(event: CustomEvent) {
    dispatch('reveal', event.detail);
  }

  function handleTabSummariesChange(event: CustomEvent<TabSummary[]>) {
    tabSummaries = event.detail;
  }

  function handleActiveTabIdChange(event: CustomEvent<string>) {
    activeTabId = event.detail;
  }

  function handleRuntimeState(event: CustomEvent<RuntimeStateEventDetail>) {
    dispatch('runtime-state', event.detail);
  }
</script>

<EditorCore
  bind:this={editorCore}
  bind:tabSummaries
  bind:activeTabId
  {enableRevealSync}
  {synchronizedRuntimeLoading}
  {onScroll}
  on:reveal={handleReveal}
  on:runtime-state={handleRuntimeState}
/>
