/**
 * useStreamingMessage — Manage streaming message state.
 *
 * Tracks the current streaming message's content and status,
 * providing helpers for appending chunks and finalizing.
 */

import { useState, useCallback, useRef } from "react";

interface StreamingState {
  /** Whether we're currently streaming */
  isStreaming: boolean;
  /** Current accumulated content */
  content: string;
  /** Unique ID of the streaming message */
  messageId: string | null;
}

interface UseStreamingMessageReturn {
  /** Current streaming state */
  state: StreamingState;
  /** Start a new streaming message */
  startStream: (messageId: string) => void;
  /** Append a text chunk to the current stream */
  appendChunk: (chunk: string) => void;
  /** Finalize the stream (mark as complete) */
  endStream: () => string;
  /** Abort the current stream */
  abortStream: () => void;
}

/**
 * Hook for managing the lifecycle of a single streaming message.
 *
 * Usage:
 * ```tsx
 * const { state, startStream, appendChunk, endStream } = useStreamingMessage();
 *
 * // When agent starts responding:
 * startStream("msg-123");
 *
 * // On each token:
 * appendChunk("Hello ");
 * appendChunk("world!");
 *
 * // When done:
 * const finalContent = endStream();
 * ```
 */
export function useStreamingMessage(): UseStreamingMessageReturn {
  const [state, setState] = useState<StreamingState>({
    isStreaming: false,
    content: "",
    messageId: null,
  });

  // Use a ref for content accumulation to avoid stale closures in rapid updates
  const contentRef = useRef("");

  const startStream = useCallback((messageId: string) => {
    contentRef.current = "";
    setState({
      isStreaming: true,
      content: "",
      messageId,
    });
  }, []);

  const appendChunk = useCallback((chunk: string) => {
    contentRef.current += chunk;
    setState((prev) => ({
      ...prev,
      content: contentRef.current,
    }));
  }, []);

  const endStream = useCallback((): string => {
    const finalContent = contentRef.current;
    setState({
      isStreaming: false,
      content: finalContent,
      messageId: null,
    });
    return finalContent;
  }, []);

  const abortStream = useCallback(() => {
    setState((prev) => ({
      ...prev,
      isStreaming: false,
    }));
  }, []);

  return { state, startStream, appendChunk, endStream, abortStream };
}
