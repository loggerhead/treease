import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  type FileAccessGrant,
  parseDesktopDeepLinks,
  type WorkspaceHost,
  type WorkspaceCommand,
  type WorkspaceOpenFileOptions,
  type WorkspaceSession,
  type WorkspaceSaveTextOptions,
} from './contract';

let pendingUpdate: Update | null = null;

export const desktopWorkspaceHost: WorkspaceHost = {
  surface: 'desktop',
  async openFile(_options: WorkspaceOpenFileOptions) {
    const document = await invoke<{ grant: FileAccessGrant; text: string } | null>('pick_file');
    if (!document) return null;
    return Object.assign(new File([document.text], document.grant.name, { type: 'text/plain;charset=utf-8' }), {
      fileAccessGrant: document.grant,
    });
  },
  async saveText(options: WorkspaceSaveTextOptions) {
    await this.saveFileAs(options);
  },
  async readFile(grant) {
    const document = await invoke<{ text: string }>('read_granted_file', { grantId: grant.id });
    return { text: document.text };
  },
  async saveFile(grant, text) {
    await invoke('save_granted_file', { grantId: grant.id, text });
  },
  async saveFileAs(options) {
    return invoke<FileAccessGrant | null>('save_new_file', { fileName: options.fileName, text: options.text });
  },
  async watchFile(grant, onChange) {
    await invoke('watch_granted_file', { grantId: grant.id });
    const unlisten: UnlistenFn = await listen<{ grantId: string }>('workspace-file-changed', (event) => {
      if (event.payload.grantId === grant.id) onChange(event.payload);
    });
    return async () => {
      unlisten();
      await invoke('unwatch_granted_file', { grantId: grant.id });
    };
  },
  async listRecentFiles() {
    return invoke<FileAccessGrant[]>('list_recent_files');
  },
  async openRecentFile(grant) {
    const document = await invoke<{ grant: FileAccessGrant; text: string }>('open_recent_file', { recentId: grant.id });
    return Object.assign(new File([document.text], document.grant.name, { type: 'text/plain;charset=utf-8' }), {
      fileAccessGrant: document.grant,
    });
  },
  async clearRecentFiles() {
    await invoke('clear_recent_files');
  },
  async onFilesDropped(onFiles) {
    return listen<Array<{ grant: FileAccessGrant; text: string }>>('workspace-files-dropped', (event) => {
      onFiles(event.payload.map((document) => Object.assign(
        new File([document.text], document.grant.name, { type: 'text/plain;charset=utf-8' }),
        { fileAccessGrant: document.grant },
      )));
    });
  },
  async takeStartupFiles() {
    const files = await invoke<Array<{ grant: FileAccessGrant; text: string }>>('take_startup_files');
    return files.map((document) => Object.assign(
      new File([document.text], document.grant.name, { type: 'text/plain;charset=utf-8' }),
      { fileAccessGrant: document.grant },
    ));
  },
  async saveSession(session: WorkspaceSession) {
    await invoke('save_workspace_session', { session });
  },
  async loadSession() {
    return invoke<WorkspaceSession | null>('load_workspace_session');
  },
  async onCommand(onCommand) {
    return listen<WorkspaceCommand>('workspace-command', (event) => onCommand(event.payload));
  },
  async storeRefreshToken(refreshToken) {
    await invoke('store_refresh_token', { refreshToken });
  },
  async hasRefreshToken() {
    return invoke<boolean>('has_refresh_token');
  },
  async refreshAccessToken(supabaseUrl, anonKey) {
    return invoke<string>('refresh_access_token', { supabaseUrl, anonKey });
  },
  async checkForUpdate() {
    pendingUpdate = await check();
    return pendingUpdate ? { version: pendingUpdate.version } : null;
  },
  async installCheckedUpdate() {
    if (!pendingUpdate) throw new Error('No checked desktop update is available.');
    await pendingUpdate.downloadAndInstall();
    pendingUpdate = null;
  },
  async clearRefreshToken() {
    await invoke('clear_refresh_token');
  },
  async openExternal(url: URL) {
    await invoke('open_external_url', { url: url.toString() });
  },
  async getInitialDeepLinks() {
    return parseDesktopDeepLinks((await getCurrent()) ?? []);
  },
  async onDeepLinks(onUrls) {
    return onOpenUrl((urls) => onUrls(parseDesktopDeepLinks(urls)));
  },
};
