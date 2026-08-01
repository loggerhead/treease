import type {
  FullEditSessionKind,
  FullEditTransportKind,
  FullEditUiPhase,
  FullEditUiState,
} from './editor-store-types';

export const initialFullEditUiState: FullEditUiState = {
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
};

export type {
  FullEditSessionKind,
  FullEditTransportKind,
  FullEditUiPhase,
  FullEditUiState,
};
