import { derived } from 'svelte/store';

import { activeTempModel } from './graph-selection-store';

export { activeTempModel } from './graph-selection-store';

export const diagnostics = derived(activeTempModel, ($tempModel) => $tempModel.diagnostics);
export const status = derived(activeTempModel, ($tempModel) => $tempModel.status);
export const cursor = derived(activeTempModel, ($tempModel) => $tempModel.cursor);
export const selectionLength = derived(activeTempModel, ($tempModel) => $tempModel.selectionLength);

export type {
  DiagnosticContextLine,
  DiagnosticItem,
  TempModel,
} from './editor-store-types';
