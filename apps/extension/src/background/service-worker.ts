import { TAB_DATA_TTL_MS } from '../shared/constants';
import { isExtensionMessage, type ExtensionMessage } from '../shared/messages';
import { detectStructuredCandidate } from './json-detection';
import { getSettings, isOriginEnabled, updateSettings } from '../shared/settings';
import type { CandidatePayload, PanelState, ReadyDocument } from '../shared/types';
import { TabStateStore } from './tab-state';

const stateByTabId = new TabStateStore();

function emptyState(): PanelState {
  return { status: 'empty' };
}

function currentState(tabId: number): PanelState {
  return stateByTabId.get(tabId);
}

function postState(tabId: number): void {
  chrome.runtime.sendMessage({ type: 'panel-state', state: currentState(tabId) } satisfies ExtensionMessage).catch(() => {
    // No panel is listening; state remains available for an explicit action click.
  });
}

async function tryOpenPanel(tabId: number): Promise<boolean> {
  try {
    await chrome.sidePanel.open({ tabId });
    return true;
  } catch {
    return false;
  }
}

function notifyOpenFallback(tabId: number): void {
  chrome.tabs.sendMessage(tabId, { type: 'treease-panel-open-fallback' }).catch(() => {
    // The page can have navigated before the fallback hint is sent.
  });
}

async function processCandidate(
  tabId: number,
  payload: CandidatePayload,
  openMode: 'user-gesture' | 'auto',
  panelOpening?: Promise<void>,
): Promise<void> {
  const settings = await getSettings();
  if (!isOriginEnabled(settings, payload.pageOrigin)) return;
  const detected = detectStructuredCandidate(payload.text);
  if (detected.status === 'invalid') {
    stateByTabId.set(tabId, {
      status: 'invalid', message: detected.message, position: detected.position,
      pageTitle: payload.pageTitle, pageOrigin: payload.pageOrigin,
    });
    postState(tabId);
    if (!await openPanelForCandidate(tabId, openMode, panelOpening)) notifyOpenFallback(tabId);
    return;
  }
  stateByTabId.set(tabId, {
    status: 'ready',
    document: {
      ...payload,
      text: detected.text,
      sourceLength: new TextEncoder().encode(detected.text).byteLength,
      rootType: detected.status === 'valid' ? detected.rootType : 'object',
      language: detected.language,
      candidateKind: detected.candidateKind,
      expiresAt: Date.now() + TAB_DATA_TTL_MS,
    } satisfies ReadyDocument,
  });
  postState(tabId);
  if (!await openPanelForCandidate(tabId, openMode, panelOpening)) notifyOpenFallback(tabId);
}

async function openPanelForCandidate(tabId: number, openMode: 'user-gesture' | 'auto', panelOpening?: Promise<void>): Promise<boolean> {
  if (openMode === 'user-gesture') return await panelOpening?.then(() => true).catch(() => false) ?? false;
  // Chrome may reject this because a document-load message has no user gesture.
  // Keep the JSON in tab-scoped memory and fall back to the action icon if so.
  return await tryOpenPanel(tabId);
}

chrome.runtime.onInstalled.addListener(() => {
  void chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: true });
});

async function activeTabId(): Promise<number | null> {
  const tabs = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  return tabs[0]?.id ?? null;
}

chrome.runtime.onMessage.addListener((rawMessage: unknown, sender, sendResponse) => {
  if (!isExtensionMessage(rawMessage)) return;
  const message = rawMessage as ExtensionMessage;
  const senderTabId = sender.tab?.id;
  if (message.type === 'candidate' && senderTabId != null) {
    // Chrome requires this API call to stay in the click-triggered message turn. Do not await
    // storage or parsing before it, or the native panel loses its user-gesture authorization.
    const panelOpening = message.openMode === 'user-gesture' ? chrome.sidePanel.open({ tabId: senderTabId }) : undefined;
    void processCandidate(senderTabId, { ...message.payload, frameId: sender.frameId }, message.openMode, panelOpening).then(() => sendResponse({ ok: true }));
    return true;
  }
  if (message.type === 'candidate-too-large' && senderTabId != null) {
    stateByTabId.set(senderTabId, { status: 'too_large', ...message.payload });
    postState(senderTabId);
    if (message.openMode === 'auto') void tryOpenPanel(senderTabId).then((opened) => { if (!opened) notifyOpenFallback(senderTabId); });
    sendResponse({ ok: true });
    return;
  }
  if (message.type === 'get-panel-state') {
    void (senderTabId == null ? activeTabId() : Promise.resolve(senderTabId)).then((tabId) => {
      sendResponse({ type: 'panel-state', state: tabId == null ? emptyState() : currentState(tabId) } satisfies ExtensionMessage);
    });
    return true;
  }
  if (message.type === 'get-settings') {
    void getSettings().then((settings) => sendResponse({ type: 'settings', settings } satisfies ExtensionMessage));
    return true;
  }
  if (message.type === 'update-settings') {
    void updateSettings(message.patch).then((settings) => sendResponse({ type: 'settings', settings } satisfies ExtensionMessage));
    return true;
  }
  if (message.type === 'open-panel') {
    void (senderTabId == null ? activeTabId() : Promise.resolve(senderTabId)).then(async (tabId) => {
      sendResponse({ opened: tabId == null ? false : await tryOpenPanel(tabId) });
    });
    return true;
  }
});

chrome.tabs.onRemoved.addListener((tabId) => stateByTabId.clear(tabId));
chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.status === 'loading') stateByTabId.clear(tabId);
});
