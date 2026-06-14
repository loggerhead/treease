import { settingsStore } from '../settings/settings-store';
import { registerTreeaseSettingsBridge } from './window-treease';

export function installSettingsBridge(): void {
  registerTreeaseSettingsBridge({
    getState: () => settingsStore.get(),
    getStatus: () => settingsStore.get().status,
    save: (settings) => settingsStore.save(settings),
    saveDocument: (document) => settingsStore.saveDocument(document),
    reset: () => settingsStore.reset(),
  });
}
