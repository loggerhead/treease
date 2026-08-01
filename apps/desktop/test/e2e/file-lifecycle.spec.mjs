describe('Desktop stable workflow and host boundaries', () => {
  it('Desktop-specific: keeps file actions in the native host instead of the Web toolbar', async () => {
    const pageActionIds = [
      'topbar-open-document',
      'topbar-save-document',
      'topbar-save-as-document',
    ];
    for (const testId of pageActionIds) {
      expect(await (await browser.$(`[data-testid="${testId}"]`)).isExisting()).toBe(false);
    }
    expect(await (await browser.$('[aria-label="Recent documents"]')).isExisting()).toBe(false);
    expect(await (await browser.$('[data-testid="topbar-import-button"]')).isExisting()).toBe(true);
    expect(await (await browser.$('[data-testid="topbar-export-button"]')).isExisting()).toBe(true);
  });

  it('Stable workflow: starts the real Tauri editor and applies an incremental edit', async () => {
    const editor = await browser.$('[data-testid="editor-tab-strip"]');
    await editor.waitForDisplayed();

    await browser.waitUntil(async () => browser.execute(() => window._treease?.editor?.isReady('source-editor') === true));
    const currentText = await browser.execute(() => window._treease.editor.getValue('source-editor'));
    const lines = currentText.split('\n');
    const lineNumber = lines.length;
    const columnNumber = lines.at(-1).length + 1;
    await browser.execute((line, column) => {
      window._treease.editor.applyEdits('source-editor', [{
        range: {
          startLineNumber: line,
          startColumn: column,
          endLineNumber: line,
          endColumn: column,
        },
        text: 'desktop-smoke',
      }]);
    }, lineNumber, columnNumber);

    await browser.waitUntil(async () => {
      const after = await browser.execute(() => window._treease.editor.getValue('source-editor'));
      return after.includes('desktop-smoke');
    });
  });

  it('Desktop-specific: round-trips the host-owned workspace recovery session through IPC', async () => {
    const expected = {
      version: 1,
      activeTabIndex: 0,
      tabs: [{
        name: 'Desktop smoke draft',
        languageId: 'json',
        sourceText: '{"desktopSmoke":true}',
        origin: 'user',
        savedText: '{"desktopSmoke":true}',
      }],
    };

    await browser.tauri.execute(({ core }, session) => core.invoke('save_workspace_session', { session }), expected);
    const actual = await browser.tauri.execute(({ core }) => core.invoke('load_workspace_session'));

    expect(actual).toEqual(expected);
  });
});
