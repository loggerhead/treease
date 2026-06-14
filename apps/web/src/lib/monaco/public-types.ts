import type * as Monaco from 'monaco-editor';

export type MonacoApi = typeof Monaco;
export type MonacoEditor = Monaco.editor.IStandaloneCodeEditor;
export type MonacoModel = Monaco.editor.ITextModel;
export type MonacoDisposable = Monaco.IDisposable;
