export type WorkspaceSurface = 'browser' | 'desktop';

export type WorkspaceOpenFileOptions = {
  accept: string[];
};

/** An opaque capability for one file that the user explicitly selected. */
export type FileAccessGrant = {
  id: string;
  name: string;
};

export type WorkspaceOpenedFile = File & {
  fileAccessGrant?: FileAccessGrant;
};

export type WorkspaceFileChange = {
  grantId: string;
};

export type RecentWorkspaceFile = FileAccessGrant;

/** Host-owned crash-recovery data. Linked files are restored as drafts, never reopened implicitly. */
export type WorkspaceSession = {
  version: 1;
  activeTabIndex: number;
  tabs: Array<{
    name: string;
    languageId: string;
    sourceText: string;
    savedText?: string;
    linkedFileName?: string;
  }>;
};

export type WorkspaceCommand =
  | 'workspace:new'
  | 'workspace:open'
  | 'workspace:save'
  | 'workspace:save-as'
  | 'workspace:import'
  | 'workspace:export'
  | 'workspace:clear-recent'
  | `workspace:open-recent:${string}`
  | 'workspace:close-tab'
  | 'workspace:toggle-viewer'
  | 'workspace:help';

export type WorkspaceSaveTextOptions = {
  fileName: string;
  text: string;
  mimeType: string;
};

export type WorkspaceHost = {
  readonly surface: WorkspaceSurface;
  openFile(options: WorkspaceOpenFileOptions): Promise<WorkspaceOpenedFile | null>;
  saveText(options: WorkspaceSaveTextOptions): Promise<void>;
  readFile(grant: FileAccessGrant): Promise<{ text: string }>;
  saveFile(grant: FileAccessGrant, text: string): Promise<void>;
  saveFileAs(options: WorkspaceSaveTextOptions): Promise<FileAccessGrant | null>;
  watchFile(grant: FileAccessGrant, onChange: (change: WorkspaceFileChange) => void): Promise<() => void>;
  listRecentFiles(): Promise<RecentWorkspaceFile[]>;
  openRecentFile(grant: FileAccessGrant): Promise<WorkspaceOpenedFile | null>;
  clearRecentFiles(): Promise<void>;
  onFilesDropped(onFiles: (files: WorkspaceOpenedFile[]) => void): Promise<() => void>;
  takeStartupFiles(): Promise<WorkspaceOpenedFile[]>;
  saveSession(session: WorkspaceSession): Promise<void>;
  loadSession(): Promise<WorkspaceSession | null>;
  onCommand(onCommand: (command: WorkspaceCommand) => void): Promise<() => void>;
  storeRefreshToken(refreshToken: string): Promise<void>;
  hasRefreshToken(): Promise<boolean>;
  refreshAccessToken(supabaseUrl: string, anonKey: string): Promise<string>;
  checkForUpdate(): Promise<{ version: string } | null>;
  installCheckedUpdate(): Promise<void>;
  clearRefreshToken(): Promise<void>;
  openExternal(url: URL): Promise<void>;
  getInitialDeepLinks(): Promise<URL[]>;
  onDeepLinks(onUrls: (urls: URL[]) => void): Promise<() => void>;
};

export class WorkspaceHostUnavailableError extends Error {
  constructor(operation: string) {
    super(`${operation} is not configured for this workspace host.`);
    this.name = 'WorkspaceHostUnavailableError';
  }
}

export function parseEditorDeepLinks(values: readonly string[]): URL[] {
  return values.flatMap((value) => {
    try {
      const url = new URL(value);
      return url.protocol === 'treease:' && url.hostname === 'editor' ? [url] : [];
    } catch {
      return [];
    }
  });
}

export function parseDesktopDeepLinks(values: readonly string[]): URL[] {
  return values.flatMap((value) => {
    try {
      const url = new URL(value);
      const isEditor = url.protocol === 'treease:' && url.hostname === 'editor';
      const isAuthCallback = url.protocol === 'treease:' && url.hostname === 'auth' && url.pathname === '/callback';
      return isEditor || isAuthCallback ? [url] : [];
    } catch {
      return [];
    }
  });
}
