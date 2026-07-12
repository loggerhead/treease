describe('Desktop file lifecycle', () => {
  it('starts the real Tauri editor and creates independent tabs', async () => {
    const editor = await browser.$('[data-testid="editor-tab-strip"]');
    await editor.waitForDisplayed();

    const before = await browser.$$('[data-testid="editor-tab"]');
    await (await browser.$('[data-testid="new-tab-button"]')).click();
    await browser.waitUntil(async () => (await browser.$$('[data-testid="editor-tab"]')).length === before.length + 1);
  });

  it('reads the host-owned workspace recovery session through real Tauri IPC', async () => {
    const session = await browser.tauri.execute(({ core }) => core.invoke('load_workspace_session'));
    if (session !== null && (typeof session !== 'object' || session.version !== 1)) {
      throw new Error('Desktop recovery session must be null or a version 1 session.');
    }
  });
});
