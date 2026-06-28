type SidecarExternalSyncState = {
  acceptedText: string;
  focused: boolean;
  dirty: boolean;
};

export function createSidecarExternalSync(initialText = '') {
  const state: SidecarExternalSyncState = {
    acceptedText: initialText,
    focused: false,
    dirty: false,
  };

  function updateDirty(modelText: string): void {
    state.dirty = modelText !== state.acceptedText;
  }

  return {
    focus(): void {
      state.focused = true;
    },
    blur(): void {
      state.focused = false;
    },
    recordLocalText(modelText: string): void {
      updateDirty(modelText);
    },
    shouldApplyExternalText(externalText: string, modelText: string): boolean {
      if (externalText === modelText) {
        state.acceptedText = externalText;
        updateDirty(modelText);
        return false;
      }
      if (state.focused || state.dirty || modelText !== state.acceptedText) {
        updateDirty(modelText);
        return false;
      }
      return true;
    },
    acceptExternalText(externalText: string): void {
      state.acceptedText = externalText;
      state.dirty = false;
    },
    reset(initialText: string): void {
      state.acceptedText = initialText;
      state.focused = false;
      state.dirty = false;
    },
    snapshot(): Readonly<SidecarExternalSyncState> {
      return { ...state };
    },
  };
}
