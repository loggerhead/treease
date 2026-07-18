const LEMON_SQUEEZY_SCRIPT_URL = "https://app.lemonsqueezy.com/js/lemon.js";

type LemonSqueezyApi = {
  Url: {
    Open(url: string): void;
  };
};

type LemonSqueezyWindow = Window & {
  LemonSqueezy?: LemonSqueezyApi;
  createLemonSqueezy?: () => void;
};

let loadingScript: Promise<LemonSqueezyApi> | null = null;

function readApi(): LemonSqueezyApi | null {
  return (window as LemonSqueezyWindow).LemonSqueezy ?? null;
}

function loadScript(): Promise<LemonSqueezyApi> {
  const api = readApi();
  if (api) return Promise.resolve(api);
  if (loadingScript) return loadingScript;

  loadingScript = new Promise<LemonSqueezyApi>((resolve, reject) => {
    const handleLoad = () => {
      (window as LemonSqueezyWindow).createLemonSqueezy?.();
      const loadedApi = readApi();
      if (!loadedApi) {
        reject(new Error("Lemon Squeezy checkout did not initialize."));
        return;
      }

      resolve(loadedApi);
    };
    const handleError = () => reject(new Error("Unable to load secure checkout."));
    const existing = document.querySelector<HTMLScriptElement>(
      `script[src="${LEMON_SQUEEZY_SCRIPT_URL}"]`,
    );

    if (existing) {
      existing.addEventListener("load", handleLoad, { once: true });
      existing.addEventListener("error", handleError, { once: true });
      return;
    }

    const script = document.createElement("script");
    script.src = LEMON_SQUEEZY_SCRIPT_URL;
    script.async = true;
    script.addEventListener("load", handleLoad, { once: true });
    script.addEventListener("error", handleError, { once: true });
    document.head.appendChild(script);
  }).catch((error: unknown) => {
    loadingScript = null;
    throw error;
  });

  return loadingScript;
}

/** Loads the provider overlay before a checkout URL is available. */
export async function preloadLemonSqueezyCheckout(): Promise<void> {
  if (typeof window === "undefined" || typeof document === "undefined") return;
  await loadScript();
}

/** Opens the provider-hosted checkout so payment details never reach Treease. */
export async function openLemonSqueezyCheckout(checkoutUrl: string): Promise<void> {
  if (typeof window === "undefined" || typeof document === "undefined") {
    throw new Error("Secure checkout is only available in a browser.");
  }

  const api = await loadScript();
  api.Url.Open(checkoutUrl);
}
