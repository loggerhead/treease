import { installEditorStoreBridge } from './register-editor-bridge';
import { installGraphExtrasBridge } from './register-graph-bridge';
import { installPreviewBridge } from './register-preview-bridge';
import { installSettingsBridge } from './register-settings-bridge';
import { installWorkerBridge } from './register-worker-bridge';
import { ensureWindowTreease } from './window-treease';

let installed = false;

export function installTestBridge(): void {
  if (installed) return;
  installed = true;
  ensureWindowTreease();
  installEditorStoreBridge();
  installSettingsBridge();
  installWorkerBridge();
  installPreviewBridge();
  installGraphExtrasBridge();
}
