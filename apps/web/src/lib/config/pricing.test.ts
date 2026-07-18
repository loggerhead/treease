import { describe, expect, it } from "vitest";

import { pricingConfig } from "./pricing";

describe("pricingConfig", () => {
  it("maps each paid plan to its server-side checkout price", () => {
    expect(pricingConfig.plans.find((plan) => plan.id === "pro-monthly")?.billingPriceId).toBe(
      "monthly",
    );
    expect(pricingConfig.plans.find((plan) => plan.id === "pro-yearly")?.billingPriceId).toBe(
      "yearly",
    );
  });

  it("keeps yearly Pro features aligned with monthly Pro", () => {
    const monthly = pricingConfig.plans.find((plan) => plan.id === "pro-monthly");
    const yearly = pricingConfig.plans.find((plan) => plan.id === "pro-yearly");

    expect(yearly?.features).toBe(monthly?.features);
  });
});
