/**
 * Shared error-handling utilities.
 * Provides structured error logging and reporting hooks.
 */

export interface ErrorContext {
  component: string;
  operation: string;
  metadata?: Record<string, unknown>;
}

/**
 * Handle an error with structured logging and optional reporting.
 * @param error - Caught error
 * @param context - Error context with component, operation, and optional metadata
 */
export function handleError(error: unknown, context: ErrorContext): void {
  const err = error instanceof Error ? error : new Error(String(error));

  console.error(`[${context.component}] ${context.operation} failed`, {
    error: err.message,
    stack: err.stack,
    ...context.metadata,
  });

  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('app:error', {
      detail: { error: err, context },
    }));
  }
}
