/** @vitest-environment happy-dom */

import { afterEach, describe, expect, it, vi } from "vitest";

import { openLemonSqueezyCheckout } from "./lemon-squeezy-checkout";

type TestWindow = Window & {
  LemonSqueezy?: {
    Url: {
      Open(url: string): void;
    };
  };
  createLemonSqueezy?: () => void;
};

afterEach(() => {
  delete (window as TestWindow).LemonSqueezy;
  delete (window as TestWindow).createLemonSqueezy;
});

describe("openLemonSqueezyCheckout", () => {
  it("initializes the API after loading the provider script", async () => {
    const open = vi.fn();
    const appendChild = vi.spyOn(document.head, "appendChild").mockImplementation((node) => node);
    (window as TestWindow).createLemonSqueezy = () => {
      (window as TestWindow).LemonSqueezy = { Url: { Open: open } };
    };

    const opening = openLemonSqueezyCheckout("https://billing.example.com/checkout/buy/monthly");
    const script = appendChild.mock.calls[0]?.[0] as HTMLScriptElement;
    script.dispatchEvent(new Event("load"));

    await opening;

    expect(open).toHaveBeenCalledWith("https://billing.example.com/checkout/buy/monthly");
    appendChild.mockRestore();
  });
});
