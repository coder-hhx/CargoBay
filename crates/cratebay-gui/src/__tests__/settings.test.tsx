import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ReasoningEffort } from "@/components/settings/ReasoningEffort";
import { useSettingsStore } from "@/stores/settingsStore";

// Mock @tauri-apps/api to avoid native module errors
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

describe("ReasoningEffort", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      settings: {
        language: "en",
        theme: "dark",
        sendOnEnter: true,
        showAgentThinking: true,
        maxConversationHistory: 50,
        containerDefaultTtlHours: 8,
        confirmDestructiveOps: true,
        reasoningEffort: "medium",
      },
    });
  });

  it("renders all three reasoning levels", () => {
    render(<ReasoningEffort />);

    expect(screen.getByText("Low")).toBeInTheDocument();
    expect(screen.getByText("Medium")).toBeInTheDocument();
    expect(screen.getByText("High")).toBeInTheDocument();
  });

  it("renders the label and description", () => {
    render(<ReasoningEffort />);

    expect(screen.getByText("Reasoning Effort")).toBeInTheDocument();
    expect(
      screen.getByText(/Controls how much reasoning/),
    ).toBeInTheDocument();
  });

  it("clicking a level updates the settings store", () => {
    render(<ReasoningEffort />);

    fireEvent.click(screen.getByText("High"));
    expect(useSettingsStore.getState().settings.reasoningEffort).toBe("high");

    fireEvent.click(screen.getByText("Low"));
    expect(useSettingsStore.getState().settings.reasoningEffort).toBe("low");
  });
});
