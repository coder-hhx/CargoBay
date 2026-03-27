import { useEffect, useCallback } from "react";
import { cn } from "@/lib/utils";
import { X } from "lucide-react";

interface SlidePanelProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
  width?: number;
}

/**
 * Right-side slide-out panel that sits INSIDE the main content area
 * (below TopBar, above StatusBar). Uses absolute positioning within
 * its parent flex container.
 */
export function SlidePanel({
  isOpen,
  onClose,
  title,
  children,
  width = 420,
}: SlidePanelProps) {
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  useEffect(() => {
    if (isOpen) {
      document.addEventListener("keydown", handleKeyDown);
      return () => document.removeEventListener("keydown", handleKeyDown);
    }
  }, [isOpen, handleKeyDown]);

  return (
    <>
      {/* Minimal overlay — allows background interaction, transparent */}
      {isOpen && (
        <div
          className="absolute inset-0 z-40 pointer-events-none"
          aria-hidden="true"
        />
      )}

      {/* Panel — slides in from the right within the content area */}
      <div
        className={cn(
          "absolute bottom-0 right-0 top-0 z-50 flex flex-col border-l border-border bg-card shadow-2xl transition-transform duration-300 ease-in-out",
          isOpen ? "translate-x-0" : "translate-x-full",
        )}
        style={{ width: `${width}px` }}
      >
        {/* Header */}
        {title !== undefined && (
          <div className="flex items-center justify-between border-b border-border px-5 py-4">
            <h2 className="truncate text-base font-semibold text-foreground">{title}</h2>
            <button
              onClick={onClose}
              className="flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        )}

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto">
          {children}
        </div>
      </div>
    </>
  );
}
