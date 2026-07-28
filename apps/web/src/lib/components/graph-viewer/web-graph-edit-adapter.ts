import {
  createGraphValueEditController,
  type GraphValueEditControllerDeps,
} from './graph-value-edit';

/**
 * Web-only boundary for graph edits. It deliberately owns document/revision and
 * entitlement dependencies; the shared canvas runtime never receives them.
 */
export function createWebGraphEditAdapter(deps: GraphValueEditControllerDeps) {
  const controller = createGraphValueEditController(deps);
  return {
    applyGraphEdit: controller.applyGraphEdit,
    bindRuntimeEditor: controller.bindGraphEditorLifecycle,
    hasActiveEdit: controller.hasActiveEdit,
    resetActiveEditState: controller.resetActiveEditState,
  };
}
