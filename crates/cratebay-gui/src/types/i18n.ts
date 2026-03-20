/**
 * Typesafe i18n type definitions for CrateBay.
 *
 * Matches frontend-spec.md §9 — i18n Strategy.
 * All locale files must satisfy this interface.
 */

export interface Translations {
  common: {
    confirm: string;
    cancel: string;
    save: string;
    delete: string;
    loading: string;
    error: string;
  };
  chat: {
    newSession: string;
    placeholder: string;
    sendButton: string;
    thinking: string;
    toolExecuting: string;
  };
  containers: {
    title: string;
    create: string;
    start: string;
    stop: string;
    delete: string;
    noContainers: string;
  };
  mcp: {
    title: string;
    addServer: string;
    connected: string;
    disconnected: string;
  };
  settings: {
    title: string;
    general: string;
    providers: string;
    advanced: string;
    language: string;
    theme: string;
  };
}
