import path from 'node:path';
import { fileURLToPath } from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
// The repository uses a top-level Cargo workspace, so Cargo writes desktop
// binaries to the repository root target directory rather than src-tauri/target.
const application = path.join(directory, '..', '..', 'target', 'debug', 'Treease');

export const config = {
  runner: 'local',
  specs: ['./test/e2e/**/*.spec.mjs'],
  maxInstances: 1,
  services: [['@wdio/tauri-service', {
    appBinaryPath: application,
    driverProvider: 'embedded',
    windowLabel: 'main',
    clearMocks: false,
  }]],
  capabilities: [{ browserName: 'tauri', 'tauri:options': { application } }],
  logLevel: 'warn',
  waitforTimeout: 15_000,
  connectionRetryTimeout: 90_000,
  connectionRetryCount: 3,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 90_000 },
};
