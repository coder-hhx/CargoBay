/**
 * useTauriEvent — Subscribe to Tauri events with automatic cleanup.
 *
 * Manages the async unlisten lifecycle so components don't leak listeners.
 */

import { useEffect, useRef, useCallback } from "react";
import { listen } from "@/lib/tauri";

/**
 * Subscribe to a Tauri event. The listener is automatically cleaned up
 * when the component unmounts or when the event name changes.
 *
 * @param eventName - The Tauri event name to listen for
 * @param handler - Callback invoked with the event payload
 */
export function useTauriEvent<T>(
  eventName: string | null,
  handler: (payload: T) => void,
): void {
  // Use a ref to always have the latest handler without re-subscribing
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    if (eventName === null) return;

    let cancelled = false;
    let unlistenFn: (() => void) | undefined;

    void listen<T>(eventName, (payload) => {
      if (!cancelled) {
        handlerRef.current(payload);
      }
    }).then((unlisten) => {
      if (cancelled) {
        // Component already unmounted before listen resolved
        unlisten();
      } else {
        unlistenFn = unlisten;
      }
    });

    return () => {
      cancelled = true;
      unlistenFn?.();
    };
  }, [eventName]);
}

/**
 * Subscribe to a Tauri event with a dynamically generated event name.
 * Returns a tuple of [subscribe, unsubscribe] for manual control.
 */
export function useTauriEventManual<T>(): {
  subscribe: (eventName: string, handler: (payload: T) => void) => Promise<() => void>;
} {
  const activeListeners = useRef<Array<() => void>>([]);

  // Clean up all listeners on unmount
  useEffect(() => {
    return () => {
      for (const unlisten of activeListeners.current) {
        unlisten();
      }
      activeListeners.current = [];
    };
  }, []);

  const subscribe = useCallback(
    async (eventName: string, handler: (payload: T) => void): Promise<() => void> => {
      const unlisten = await listen<T>(eventName, handler);
      activeListeners.current.push(unlisten);
      return () => {
        unlisten();
        activeListeners.current = activeListeners.current.filter((fn) => fn !== unlisten);
      };
    },
    [],
  );

  return { subscribe };
}
