import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import { registerTreeaseWorkerBridge } from './window-treease';

export function installWorkerBridge(): void {
  registerTreeaseWorkerBridge({
    callShared: (type, payload, transfer) => callSharedWasmWorker(type, payload, transfer),
  });
}
