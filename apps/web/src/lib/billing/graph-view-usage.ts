const topologyVersion = 'graph-topology-v1';

/** Returns the stable monthly-usage key for Core's canonical topology bytes. */
export async function graphViewTopologyKey(topologyBytes: Uint8Array): Promise<string> {
  const bytes = topologyBytes.slice().buffer;
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  const hex = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
  return `${topologyVersion}:${hex}`;
}
