/**
 * Tauri invoke/listen wrappers with fallback for non-Tauri environments.
 *
 * When running in a browser (e.g., `pnpm dev` without Tauri), these wrappers
 * log a warning and return mock data so the UI can be developed standalone.
 */

declare global {
  interface Window {
    __TAURI__?: {
      core: {
        invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
      };
      event: {
        listen: <T>(
          event: string,
          handler: (event: { payload: T }) => void,
        ) => Promise<() => void>;
      };
    };
  }
}

/**
 * Invoke a Tauri command. Falls back to a console warning and empty result
 * when Tauri is not available (browser-only development).
 */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (window.__TAURI__) {
    return window.__TAURI__.core.invoke<T>(cmd, args);
  }
  console.warn(`[tauri-mock] invoke("${cmd}") — Tauri not available, returning mock`);
  return {} as T;
}

/**
 * Listen to a Tauri event. Falls back to a no-op unsubscribe function
 * when Tauri is not available.
 */
export async function listen<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  if (window.__TAURI__) {
    const unlisten = await window.__TAURI__.event.listen<T>(event, (e) => {
      handler(e.payload);
    });
    return unlisten;
  }
  console.warn(`[tauri-mock] listen("${event}") — Tauri not available, no-op`);
  return () => {
    /* no-op */
  };
}

/**
 * Check whether the app is running inside a Tauri webview.
 */
export function isTauri(): boolean {
  return window.__TAURI__ !== undefined;
}
