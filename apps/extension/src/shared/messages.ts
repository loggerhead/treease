import type { CandidatePayload, ExtensionSettings, PanelState } from './types';

export type ExtensionMessage =
  | { type: 'candidate'; payload: CandidatePayload; openMode: 'user-gesture' | 'auto' }
  | { type: 'candidate-too-large'; payload: Omit<CandidatePayload, 'text'>; openMode: 'user-gesture' | 'auto' }
  | { type: 'get-panel-state' }
  | { type: 'panel-state'; state: PanelState }
  | { type: 'get-settings' }
  | { type: 'settings'; settings: ExtensionSettings }
  | { type: 'update-settings'; patch: Partial<ExtensionSettings> }
  | { type: 'open-panel' };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function isExtensionMessage(value: unknown): value is ExtensionMessage {
  if (!isRecord(value) || typeof value.type !== 'string') return false;
  switch (value.type) {
    case 'get-panel-state':
    case 'get-settings':
    case 'open-panel':
      return true;
    case 'panel-state':
    case 'settings':
      return true;
    case 'candidate':
      return isCandidatePayload(value.payload) && isOpenMode(value.openMode);
    case 'candidate-too-large':
      return isCandidateMetadata(value.payload) && isOpenMode(value.openMode);
    case 'update-settings':
      return isRecord(value.patch);
    default:
      return false;
  }
}

function isOpenMode(value: unknown): value is 'user-gesture' | 'auto' {
  return value === 'user-gesture' || value === 'auto';
}

function isCandidateMetadata(value: unknown): value is Omit<CandidatePayload, 'text'> {
  return isRecord(value) && typeof value.sourceTag === 'string' && typeof value.domPath === 'string' && typeof value.sourceLength === 'number'
    && typeof value.pageTitle === 'string' && typeof value.pageOrigin === 'string';
}

function isCandidatePayload(value: unknown): value is CandidatePayload {
  return isCandidateMetadata(value) && typeof (value as Record<string, unknown>).text === 'string';
}
