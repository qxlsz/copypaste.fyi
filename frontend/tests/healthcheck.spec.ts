import { test, expect } from "@playwright/test";

test.describe("Webpage Health Check", () => {
  test("homepage loads without white screen", async ({ page }) => {
    // Navigate to the homepage
    await page.goto("http://localhost:5173");

    // Wait for the page to load
    await page.waitForLoadState("domcontentloaded");

    // Check that the body is visible (not white/empty)
    const body = page.locator("body");
    await expect(body).toBeVisible();

    // Check that there's actual content (not just empty)
    const bodyText = await body.textContent();
    expect(bodyText?.length).toBeGreaterThan(10);
  });

  test("header and navigation render correctly", async ({ page }) => {
    await page.goto("http://localhost:5173");

    // Check that the main branding is present
    await expect(
      page.getByRole("link", { name: "copypaste.fyi home" }),
    ).toBeVisible();

    // Check that navigation controls are present
    await expect(
      page.getByRole("button", { name: "Create new paste" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Service statistics" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Open command menu" }),
    ).toBeVisible();
  });

  test("paste form elements are present", async ({ page }) => {
    await page.goto("http://localhost:5173");

    // Toolbar controls
    await expect(page.getByLabel("Retention period")).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Encryption options" }),
    ).toBeVisible();

    // Check that the submit button is present
    await expect(
      page.getByRole("button", { name: "Create", exact: true }),
    ).toBeVisible();
  });

  test("encryption key input is available via the lock popover", async ({
    page,
  }) => {
    await page.goto("http://localhost:5173");

    // Open the encryption popover; the key input lives inside it.
    await page.getByRole("button", { name: "Encryption options" }).click();
    const keyInput = page.locator('input[type="password"]').first();
    await expect(keyInput).toBeAttached();
  });
});
