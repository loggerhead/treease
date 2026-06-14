import type * as Monaco from 'monaco-editor';
import type { EditorModelWithDocumentKey } from './types';

function documentKeySeed(target: Monaco.editor.ITextModel, scope?: string): string {
  return scope?.trim() || target.uri.toString();
}

function setModelDocumentKey(model: EditorModelWithDocumentKey, seed: string, version: number): string {
  const nextKey = `${seed}:${version}`;
  model.__treeaseDocumentVersion = version;
  model.__treeaseDocumentKey = nextKey;
  return nextKey;
}

export function ensureModelDocumentKey(
  target: Monaco.editor.ITextModel | null,
  scope?: string,
): string {
  if (!target) return '';
  const model = target as EditorModelWithDocumentKey;
  const seed = documentKeySeed(target, scope);
  const version = model.__treeaseDocumentVersion ?? 0;
  return setModelDocumentKey(model, seed, version);
}

export function rotateModelDocumentKey(
  target: Monaco.editor.ITextModel | null,
  scope?: string,
): string {
  if (!target) return '';
  const model = target as EditorModelWithDocumentKey;
  const seed = documentKeySeed(target, scope);
  const nextVersion = (model.__treeaseDocumentVersion ?? 0) + 1;
  return setModelDocumentKey(model, seed, nextVersion);
}
