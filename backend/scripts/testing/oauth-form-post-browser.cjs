// Gateway browser fixture only. Recording Auth is not provider/SQL acceptance.
"use strict";
const assert = require("node:assert/strict");
const path = require("node:path");

async function run() {
  const port = Number(process.argv[2]);
  assert(Number.isInteger(port) && port > 0 && port < 65536);
  const modulePath = process.env.OAUTH_BROWSER_PLAYWRIGHT;
  const executable = process.env.OAUTH_BROWSER_CHROME;
  assert(modulePath && path.isAbsolute(modulePath), "pinned-playwright-required");
  assert(executable && path.isAbsolute(executable), "pinned-chromium-required");
  const { chromium } = require(modulePath);
  const origin = `http://127.0.0.1:${port}`;
  const provider = `http://localhost:${port}/fixture/provider`;
  const browser = await chromium.launch({ executablePath: executable, headless: true });
  const contexts = new Set();
  async function context(options = {}, denial = false) {
    const value = await browser.newContext({ serviceWorkers: "block", ...options });
    contexts.add(value);
    value.setDefaultTimeout(10000);
    value.setDefaultNavigationTimeout(10000);
    await value.route("**/*", async route => {
      const request = route.request();
      const url = new URL(request.url());
      if ((url.origin === origin || url.origin === `http://localhost:${port}`) && url.protocol === "http:") {
        return route.continue();
      }
      // Follow only the actual Gateway redirect to the recording double's
      // reserved .test URL. No external DNS/provider request occurs.
      const previous = request.redirectedFrom();
      const prior = previous && new URL(previous.url());
      if (request.isNavigationRequest() && prior && prior.origin === origin &&
          prior.pathname === "/auth/oauth/apple/authorize" && url.protocol === "https:" &&
          url.hostname.endsWith(".test") && url.searchParams.getAll("state").length === 1 &&
          url.searchParams.get("state") === "apple-state") {
        return route.fulfill({ status: 302, headers: { location: provider + (denial ? "?denial=1" : "") }, body: "" });
      }
      await route.abort("blockedbyclient");
    });
    return value;
  }
  function handoff(value, status, target) {
    const url = new URL(value);
    assert.equal(url.origin, origin);
    assert.equal(url.pathname, "/auth/callback");
    assert.equal(url.hash, "");
    assert.deepEqual([...url.searchParams.entries()].sort(), [["returnTo", target], ["status", status]]);
  }
  async function authorize(value, target) {
    const page = await value.newPage();
    await page.goto(`${origin}/auth/oauth/apple/authorize?returnTo=${encodeURIComponent(target)}`);
    assert.equal(new URL(page.url()).origin, `http://localhost:${port}`);
    const state = (await value.cookies(origin)).find(cookie => cookie.name === "oauth_state_apple");
    assert(state && state.value.startsWith("v1."), "actual-signed-authorize-cookie");
    assert(state.secure && state.httpOnly && state.sameSite === "Lax" && state.domain === "127.0.0.1");
    return { page, state };
  }
  async function submit(page, status, target) {
    const completed = page.waitForResponse(response => new URL(response.url()).pathname === "/auth/oauth/apple/callback/complete");
    await page.getByRole("button", { name: "Authorize fixture", exact: true }).click();
    const response = await completed;
    assert.equal(response.status(), 303);
    const headers = await response.allHeaders();
    assert(headers["cache-control"].includes("no-store"));
    assert.equal(headers["referrer-policy"], "no-referrer");
    await page.waitForURL(url => url.pathname === "/auth/callback");
    handoff(page.url(), status, target);
  }
  try {
    const normal = await context();
    const first = await authorize(normal, "/feed");
    await submit(first.page, "success", "/feed");
    assert(!(await normal.cookies(origin)).some(cookie => cookie.name === "oauth_state_apple"));
    const refresh = (await normal.cookies(origin)).find(cookie => cookie.name === "refresh_token");
    assert(refresh && refresh.httpOnly && refresh.secure && refresh.sameSite === "Lax");
    await normal.close();
    contexts.delete(normal);

    const missing = await context();
    const missingPage = await missing.newPage();
    await missingPage.goto(provider);
    const refused = missingPage.waitForResponse(response => new URL(response.url()).pathname === "/auth/oauth/apple/callback/complete");
    await missingPage.getByRole("button", { name: "Authorize fixture", exact: true }).click();
    assert.equal((await refused).status(), 400);
    assert(!(await missing.cookies(origin)).some(cookie => cookie.name === "refresh_token"));
    await missing.close();
    contexts.delete(missing);

    const manual = await context({ javaScriptEnabled: false });
    const third = await authorize(manual, "/me");
    await third.page.getByRole("button", { name: "Authorize fixture", exact: true }).click();
    await third.page.waitForURL(url => url.pathname === "/auth/oauth/apple/callback");
    const manualDone = third.page.waitForResponse(response => new URL(response.url()).pathname === "/auth/oauth/apple/callback/complete");
    await third.page.getByRole("button", { name: "Continue sign-in", exact: true }).click();
    assert.equal((await manualDone).status(), 303);
    await third.page.waitForURL(url => url.pathname === "/auth/callback");
    handoff(third.page.url(), "success", "/me");
    await manual.close();
    contexts.delete(manual);

    const denied = await context({}, true);
    const fourth = await authorize(denied, "/world");
    await submit(fourth.page, "denied", "/world");
    assert(!(await denied.cookies(origin)).some(cookie => ["refresh_token", "oauth_state_apple"].includes(cookie.name)));
    await denied.close();
    contexts.delete(denied);

    const hostile = await context();
    const fifth = await authorize(hostile, "/feed");
    const cookie = `${fifth.state.name}=${fifth.state.value}`;
    const complete = `${origin}/auth/oauth/apple/callback/complete`;
    const form = { code: "fixture-code", state: "apple-state" };
    // These direct HTTP negatives exercise the mounted browser boundary.
    const duplicate = await hostile.request.post(complete, { headers: { origin, cookie: `${cookie}; ${cookie}` }, form, maxRedirects: 0 });
    assert.equal(duplicate.status(), 400);
    const foreign = await hostile.request.post(complete, { headers: { origin: `http://localhost:${port}`, cookie }, form, maxRedirects: 0 });
    assert.equal(foreign.status(), 403);
    const unconfigured = await hostile.request.post(`${origin}/auth/oauth/google/callback`, { headers: { cookie }, form, maxRedirects: 0 });
    assert.equal(unconfigured.status(), 403);
    process.stdout.write(JSON.stringify({ scope: "gateway-recording-auth-browser-fixture", cases: 7,
      authPersistence: "NOT_RUN", providerExchange: "NOT_RUN", canonicalRefresh: "NOT_RUN", frontendAdmission: "NOT_RUN" }));
  } finally {
    for (const value of contexts) await value.close();
    await browser.close();
  }
}

run().catch(() => {
  // Playwright failures may include HTML/cookies/codes. Keep output fixed.
  process.stderr.write("oauth-browser-fixture-failed\n");
  process.exitCode = 1;
});
