import { test, expect } from "@playwright/test";
import { installTauriMock } from "./tauri-mock";

test.describe("CrateBay App", () => {
  test("homepage loads and displays title", async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await expect(page.locator('[data-testid="app-title"]')).toBeVisible();
  });
});
