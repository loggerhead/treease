export type FullEditExternalRenderSessionRef = {
  sessionId: string;
  documentKey: string;
  language: string;
  revision: number;
};

type FullEditExternalRenderState = FullEditExternalRenderSessionRef & {
  signature: string;
};

type CompletedFullEditExternalRender = Omit<FullEditExternalRenderSessionRef, 'sessionId'>;

function matchesRenderIdentity(
  state: CompletedFullEditExternalRender | FullEditExternalRenderState | null,
  documentKey: string,
  revision: number,
  language: string,
): boolean {
  return (
    state != null &&
    state.documentKey === documentKey &&
    state.revision === revision &&
    state.language === language
  );
}

function signatureOf(ref: FullEditExternalRenderSessionRef): string {
  return `${ref.sessionId}|${ref.documentKey}|${ref.revision}|${ref.language}`;
}

export function createFullEditExternalRenderAuthority() {
  let attachedSignature = '';
  let active: FullEditExternalRenderState | null = null;
  let completed: CompletedFullEditExternalRender | null = null;

  function claim(ref: FullEditExternalRenderSessionRef): FullEditExternalRenderSessionRef | null {
    const signature = signatureOf(ref);
    if (signature === attachedSignature) return null;
    attachedSignature = signature;
    active = { ...ref, signature };
    return ref;
  }

  function release(ref: FullEditExternalRenderSessionRef): void {
    const signature = signatureOf(ref);
    if (attachedSignature === signature) attachedSignature = '';
    if (active?.signature === signature) active = null;
  }

  function markRendered(documentKey: string, revision: number, language: string): void {
    if (!matchesRenderIdentity(active, documentKey, revision, language)) return;
    completed = { documentKey, revision, language };
    active = null;
  }

  function hasActiveRender(documentKey: string, revision: number, language: string): boolean {
    return matchesRenderIdentity(active, documentKey, revision, language);
  }

  function hasCompletedRender(documentKey: string, revision: number, language: string): boolean {
    return matchesRenderIdentity(completed, documentKey, revision, language);
  }

  function reset(): void {
    attachedSignature = '';
    active = null;
    completed = null;
  }

  return {
    claim,
    release,
    markRendered,
    hasActiveRender,
    hasCompletedRender,
    reset,
  };
}
