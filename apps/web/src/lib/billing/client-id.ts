import FingerprintJS from '@fingerprintjs/fingerprintjs';

let clientIdPromise: Promise<string> | null = null;

export function getUsageClientId(): Promise<string> {
  clientIdPromise ??= FingerprintJS.load().then(async (agent) => {
    const result = await agent.get();
    const surface = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop' ? 'desktop' : 'browser';
    return `${surface}:${result.visitorId}`;
  });
  return clientIdPromise;
}
