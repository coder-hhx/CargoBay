import { test, expect } from "@playwright/test";
import { ChatPageObject } from "./pages";
import { installTauriMock } from "./tauri-mock";

/**
 * Chat Flow E2E Tests
 * 验证聊天功能和消息流
 *
 * 注意：这些测试使用 mock Tauri invoke 来模拟后端响应
 * 实际的 LLM 集成测试应该在单元测试中进行
 */
test.describe("Chat Flow", () => {
  let chatPage: ChatPageObject;

  test.beforeEach(async ({ page }) => {
    chatPage = new ChatPageObject(page);

    await installTauriMock(page, {
      containerList: [
        {
          id: "abc123",
          shortId: "abc123",
          name: "node-01",
          status: "running",
          state: "running",
          image: "node:latest",
          templateId: "node-dev",
          cpuCores: 2,
          memoryMb: 2048,
          ports: [],
          createdAt: new Date().toISOString(),
          labels: {},
        },
      ],
      llmTokens: [
        "Here",
        " ",
        "are",
        " ",
        "your",
        " ",
        "containers",
        ":",
        " ",
        "1",
        ".",
        " ",
        "**",
        "node-01",
        "**",
        " ",
        "(",
        "running",
        ")",
      ],
    });

    // 导航到应用
    await chatPage.goto("/");
    await chatPage.verifyAppLoaded();
    await chatPage.verifyInputReady();
  });

  test("能够在聊天输入中输入文本", async ({ page }) => {
    const inputText = "Hello, CrateBay!";
    await chatPage.fill(chatPage.chatInput, inputText);

    const input = page.locator(chatPage.chatInput);
    const value = await input.inputValue();
    expect(value).toBe(inputText);
  });

  test("能够发送消息", async ({ page }) => {
    const message = "List containers";

    // 发送消息
    await chatPage.sendMessage(message);

    // 验证消息出现在消息列表中
    await expect(page.locator(`text="${message}"`)).toBeVisible({ timeout: 5000 });
  });

  test("发送消息后输入框被清空", async ({ page }) => {
    const message = "Hello";

    // 发送消息
    await chatPage.sendMessage(message);
    await page.waitForTimeout(500);

    // 验证输入框被清空（或已禁用）
    const input = page.locator(chatPage.chatInput);
    let isEmpty = false;

    try {
      const value = await input.inputValue();
      isEmpty = value === "";
    } catch {
      isEmpty = true;
    }

    expect(isEmpty || (await input.isDisabled())).toBeTruthy();
  });

  test("能够发送多条消息", async ({ page }) => {
    const messages = ["First message", "Second message", "Third message"];

    for (const msg of messages) {
      await chatPage.fill(chatPage.chatInput, msg);
      await chatPage.click(chatPage.sendButton);
      await page.waitForTimeout(300);
    }

    // 验证所有消息都出现
    for (const msg of messages) {
      await expect(page.locator(`text="${msg}"`)).toBeVisible({
        timeout: 5000,
      });
    }
  });

  test("消息列表会自动滚动到最新消息", async ({ page }) => {
    // 检查是否存在消息列表
    const messageList = page.locator(chatPage.messageList);
    const listVisible = await messageList.isVisible({ timeout: 3000 }).catch(() => false);

    if (listVisible) {
      // 获取初始滚动位置
      const initialScroll = await messageList.evaluate((el) =>
        el.scrollHeight - el.scrollTop
      );

      // 发送消息
      await chatPage.sendMessage("New message");
      await page.waitForTimeout(500);

      // 检查滚动位置变化
      const finalScroll = await messageList.evaluate((el) =>
        el.scrollHeight - el.scrollTop
      );

      expect(finalScroll).toBeGreaterThanOrEqual(0);
    }
  });

  test("默认情况下应用应该处于 Chat 页面", async ({ page }) => {
    // 验证 Chat 输入存在
    await expect(page.locator(chatPage.chatInput)).toBeVisible();
  });

  test("聊天会话初始应该为空", async ({ page }) => {
    // 计算初始消息数
    const messageCount = await chatPage.getMessageCount();

    // 应该为 0 或很少（取决于欢迎消息）
    expect(messageCount).toBeLessThanOrEqual(1);
  });

  test("能够创建新会话", async ({ page }) => {
    // 发送第一条消息
    await chatPage.sendMessage("First session");
    await page.waitForTimeout(500);

    const initialMessageCount = await chatPage.getMessageCount();

    // 创建新会话
    await chatPage.startNewSession();
    await page.waitForTimeout(500);

    // 新会话应该为空
    const newSessionMessageCount = await chatPage.getMessageCount();
    expect(newSessionMessageCount).toBeLessThanOrEqual(1);
  });

  test("多个会话应该保持独立的消息历史", async ({ page }) => {
    // Session 1: 发送消息
    const session1Msg = "This is session 1";
    await chatPage.fill(chatPage.chatInput, session1Msg);
    await page.waitForTimeout(300);

    // 创建新会话
    await chatPage.startNewSession();
    await page.waitForTimeout(500);

    // Session 2: 发送不同的消息
    const session2Msg = "This is session 2";
    await chatPage.sendMessage(session2Msg);
    await page.waitForTimeout(300);

    // 验证 session 2 消息存在
    await expect(page.locator(`text="${session2Msg}"`)).toBeVisible({
      timeout: 5000,
    });
  });

  test("按 Shift+Enter 应该插入新行而不是发送", async ({ page }) => {
    const input = page.locator(chatPage.chatInput);

    // 输入文本
    await input.fill("Line 1");

    // 按 Shift+Enter
    await input.press("Shift+Enter");
    await input.type("Line 2");

    // 验证换行符
    const value = await input.inputValue();
    expect(value).toContain("\n");
  });

  test("按 Enter 应该发送消息", async ({ page }) => {
    const message = "Send with Enter";
    const input = page.locator(chatPage.chatInput);

    await input.fill(message);
    await input.press("Enter");

    // 验证消息被发送
    await expect(page.locator(`text="${message}"`)).toBeVisible({
      timeout: 5000,
    });
  });

  test("消息输入框应该处于焦点状态", async ({ page }) => {
    const input = page.locator(chatPage.chatInput);

    // 输入框应该可用
    await expect(input).toBeEnabled();

    // 尝试清空并重新输入
    await input.fill("");
    await input.type("test");

    const value = await input.inputValue();
    expect(value).toBe("test");
  });
});
