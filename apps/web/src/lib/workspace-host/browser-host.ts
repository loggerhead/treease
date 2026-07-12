import {
  WorkspaceHostUnavailableError,
  type FileAccessGrant,
  type WorkspaceHost,
  type WorkspaceOpenFileOptions,
  type WorkspaceSaveTextOptions,
} from './contract';

function chooseBrowserFile(options: WorkspaceOpenFileOptions): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = options.accept.join(',');
    input.addEventListener('change', () => resolve(input.files?.[0] ?? null), { once: true });
    input.click();
  });
}

function downloadBrowserText({ fileName, text, mimeType }: WorkspaceSaveTextOptions): void {
  const url = URL.createObjectURL(new Blob([text], { type: mimeType }));
  const link = document.createElement('a');
  link.href = url;
  link.download = fileName;
  link.click();
  URL.revokeObjectURL(url);
}

export const browserWorkspaceHost: WorkspaceHost = {
  surface: 'browser',
  openFile: chooseBrowserFile,
  async saveText(options) {
    downloadBrowserText(options);
  },
  async readFile(_grant: FileAccessGrant) {
    throw new Error('Browser files cannot be read after their initial selection.');
  },
  async saveFile(_grant: FileAccessGrant, _text: string) {
    throw new Error('Browser files must be exported through a download.');
  },
  async saveFileAs(options) {
    downloadBrowserText(options);
    return null;
  },
  async watchFile() {
    return () => {};
  },
  async listRecentFiles() {
    return [];
  },
  async openRecentFile() {
    return null;
  },
  async clearRecentFiles() {},
  async onFilesDropped() {
    return () => {};
  },
  async takeStartupFiles() {
    return [];
  },
  async saveSession() {
    throw new WorkspaceHostUnavailableError('Desktop workspace session persistence');
  },
  async loadSession() {
    throw new WorkspaceHostUnavailableError('Desktop workspace session persistence');
  },
  async onCommand() {
    return () => {};
  },
  async storeRefreshToken() {
    throw new WorkspaceHostUnavailableError('Desktop credential storage');
  },
  async hasRefreshToken() {
    return false;
  },
  async refreshAccessToken() {
    throw new WorkspaceHostUnavailableError('Desktop credential refresh');
  },
  async checkForUpdate() {
    return null;
  },
  async installCheckedUpdate() {
    throw new WorkspaceHostUnavailableError('Desktop updater');
  },
  async clearRefreshToken() {},
  async openExternal(url) {
    window.open(url, '_blank', 'noopener,noreferrer');
  },
  async getInitialDeepLinks() {
    return [];
  },
  async onDeepLinks() {
    return () => {};
  },
};
