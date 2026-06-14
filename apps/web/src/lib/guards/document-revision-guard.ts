export type DocumentRevisionGuard = {
  documentKey: string;
  revision: number;
};

type DocumentRevisionGuardInput = {
  documentKey: string;
  revision: number;
};


export function isDocumentRevisionGuardCurrent(
  guard: DocumentRevisionGuard | null,
  input: DocumentRevisionGuardInput,
): boolean {
  if (!guard) return false;
  return guard.documentKey === input.documentKey && guard.revision === input.revision;
}
