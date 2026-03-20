import { test, expect } from "@playwright/test";

test.describe("CrateBay App", () => {
  test("homepage loads and displays title", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("text=CrateBay")).toBeVisible();
  });
});
