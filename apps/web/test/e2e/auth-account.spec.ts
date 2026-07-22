import { expect, test } from "./fixtures";

test("PKCE login returns to the original page, shows the account, and supports logout", async ({
  page,
}) => {
  const expiresAt = Math.floor(Date.now() / 1000) + 3600;
  const payload = Buffer.from(
    JSON.stringify({ sub: "auth-e2e-user", exp: expiresAt })
  ).toString("base64url");
  const session = {
    access_token: `eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.${payload}.test-signature`,
    token_type: "bearer",
    expires_in: 3600,
    expires_at: expiresAt,
    refresh_token: "auth-e2e-refresh-token",
    user: {
      id: "auth-e2e-user",
      aud: "authenticated",
      role: "authenticated",
      email: "ada@example.com",
      email_confirmed_at: "2026-01-01T00:00:00Z",
      phone: "",
      confirmed_at: "2026-01-01T00:00:00Z",
      last_sign_in_at: "2026-01-01T00:00:00Z",
      app_metadata: { provider: "github", providers: ["github"] },
      user_metadata: { full_name: "Ada Lovelace" },
      identities: [],
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
      is_anonymous: false,
    },
  };

  let authorizationUrl: URL | undefined;
  let exchangeBody: Record<string, unknown> | undefined;
  await page.route("**/auth/v1/authorize**", async (route) => {
    authorizationUrl = new URL(route.request().url());
    const callback = new URL(
      authorizationUrl.searchParams.get("redirect_to") ?? ""
    );
    callback.searchParams.set("code", "auth-e2e-code");
    await route.fulfill({
      status: 302,
      headers: { location: callback.toString() },
      body: "",
    });
  });
  await page.route("**/auth/v1/token?grant_type=pkce", async (route) => {
    exchangeBody = route.request().postDataJSON() as Record<string, unknown>;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(session),
    });
  });
  await page.route("**/auth/v1/logout**", async (route) => {
    await route.fulfill({ status: 204, body: "" });
  });
  await page.route("**/v1/billing/pricing-prewarm", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ plans: [], checkouts: [] }),
    });
  });
  await page.route("**/v1/usage?clientId=*", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        tier: "free",
        periodKey: "2026-01",
        limits: {
          bidirectionalEditDocumentsMonthly: { kind: "limited", limit: 10 },
          largeFileProcessingRunsMonthly: { kind: "limited", limit: 3 },
          aiSuggestionsMonthly: { kind: "limited", limit: 0 },
          shareMaxAgeDays: 7,
        },
        usage: {},
      }),
    });
  });
  await page.route("**/v1/account", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        user: {
          id: "auth-e2e-user",
          email: "ada@example.com",
          avatarUrl: null,
        },
        subscription: {
          id: "auth-e2e-subscription",
          userId: "auth-e2e-user",
          tier: "free",
          billingCadence: null,
          status: "active",
          currentPeriodEnd: null,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        },
        usage: {
          tier: "free",
          periodKey: "2026-01",
          limits: {
            bidirectionalEditDocumentsMonthly: { kind: "limited", limit: 10 },
            largeFileProcessingRunsMonthly: { kind: "limited", limit: 3 },
            aiSuggestionsMonthly: { kind: "limited", limit: 0 },
            shareMaxAgeDays: 7,
          },
          usage: {},
        },
      }),
    });
  });

  await page.goto("/?source=pkce-e2e#pricing");
  await page.waitForLoadState("networkidle");
  await page.getByTestId("account-menu-button").click();
  await expect(page.getByTestId("account-fingerprint-id")).toContainText(
    "ID:"
  );
  await page.getByTestId("account-login-menu-item").click();
  await page.getByTestId("login-google-button").click();

  await expect(page).toHaveURL(/\/?source=pkce-e2e#pricing$/);
  expect(authorizationUrl?.searchParams.get("code_challenge")).toBeTruthy();
  expect(authorizationUrl?.searchParams.get("code_challenge_method")).toBe(
    "s256"
  );
  expect(exchangeBody).toMatchObject({ auth_code: "auth-e2e-code" });
  expect(exchangeBody?.code_verifier).toBeTruthy();
  await expect(page.getByTestId("account-avatar-button")).toHaveCSS(
    "cursor",
    "pointer"
  );
  await page.getByTestId("account-avatar-button").click();
  await expect(page.getByTestId("account-details")).toContainText(
    "Ada Lovelace"
  );
  await expect(page.getByTestId("account-details")).toContainText(
    "ada@example.com"
  );
  await expect(page.getByTestId("account-check-updates-menu-item")).toHaveCount(
    0
  );
  await page.getByTestId("account-logout-menu-item").click();
  await expect(page.getByTestId("account-menu-button")).toBeVisible();
});
