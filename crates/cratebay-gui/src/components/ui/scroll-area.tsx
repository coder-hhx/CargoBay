import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * Native ScrollArea replacement for Radix ScrollArea.
 *
 * Radix ScrollArea uses inline ref callbacks that trigger setState on every
 * render in React 19, causing "Maximum update depth exceeded" (error #185).
 * This native implementation provides the same API surface using CSS overflow.
 */
function ScrollArea({
  className,
  children,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="scroll-area"
      className={cn("relative overflow-auto", className)}
      {...props}
    >
      {children}
    </div>
  )
}

function ScrollBar({
  className,
  orientation = "vertical",
}: {
  className?: string
  orientation?: "vertical" | "horizontal"
}) {
  // Native scrollbars are used; this is a no-op placeholder for API compat
  void className
  void orientation
  return null
}

export { ScrollArea, ScrollBar }
