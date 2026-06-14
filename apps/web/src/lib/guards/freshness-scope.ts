export type VersionedModel = {
  getVersionId: () => number;
};

export type FreshnessContext = {
  documentKey?: string;
  revision?: number;
  languageId?: string;
  sessionId?: string | null;
  token?: number;
  model?: VersionedModel | null;
};

export type FreshnessScope = {
  isCurrent: () => boolean;
  step: <T>(task: () => Promise<T>) => Promise<T | null>;
};

type CapturedFreshnessContext = Omit<FreshnessContext, 'model'> & {
  model?: VersionedModel | null;
  modelVersionId?: number | null;
  checksModel: boolean;
};

function hasOwnContextKey<K extends keyof FreshnessContext>(value: FreshnessContext, key: K): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function captureFreshnessContext(context: FreshnessContext): CapturedFreshnessContext {
  const checksModel = hasOwnContextKey(context, 'model');
  return {
    ...context,
    checksModel,
    modelVersionId: checksModel && context.model ? context.model.getVersionId() : null,
  };
}

function matchesContext(captured: CapturedFreshnessContext, current: FreshnessContext): boolean {
  if (hasOwnContextKey(captured, 'documentKey') && captured.documentKey !== current.documentKey) return false;
  if (hasOwnContextKey(captured, 'revision') && captured.revision !== current.revision && current.revision != null) return false;
  if (hasOwnContextKey(captured, 'languageId') && captured.languageId !== current.languageId) return false;
  if (hasOwnContextKey(captured, 'sessionId') && captured.sessionId !== current.sessionId && current.sessionId != null) return false;
  if (hasOwnContextKey(captured, 'token') && captured.token !== current.token) return false;
  if (!captured.checksModel) return true;
  if (!captured.model || !current.model) return false;
  if (captured.model !== current.model) return false;
  return current.model.getVersionId() === captured.modelVersionId;
}

export function createFreshnessScope(
  capturedContext: FreshnessContext,
  getCurrentContext: () => FreshnessContext,
): FreshnessScope {
  const captured = captureFreshnessContext(capturedContext);
  const isCurrent = () => matchesContext(captured, getCurrentContext());
  return {
    isCurrent,
    async step<T>(task: () => Promise<T>): Promise<T | null> {
      if (!isCurrent()) return null;
      const value = await task();
      return isCurrent() ? value : null;
    },
  };
}
