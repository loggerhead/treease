/**
 * 统一错误处理工具
 * 提供结构化的错误日志和上报机制
 */

export interface ErrorContext {
  component: string;
  operation: string;
  metadata?: Record<string, unknown>;
}

/**
 * 处理错误，提供结构化日志和可选的上报机制
 * @param error - 捕获的错误对象
 * @param context - 错误上下文信息，包含组件名、操作名和可选元数据
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
