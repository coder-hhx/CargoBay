import "@testing-library/jest-dom/vitest";

// jsdom does not implement scrollIntoView — stub it globally
if (typeof HTMLElement.prototype.scrollIntoView !== "function") {
  HTMLElement.prototype.scrollIntoView = () => {};
}
