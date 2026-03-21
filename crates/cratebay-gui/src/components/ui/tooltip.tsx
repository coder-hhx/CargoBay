"use client"

import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * Native Tooltip replacement for Radix Tooltip.
 *
 * Radix Tooltip uses ref callbacks with useState that cause
 * "Maximum update depth exceeded" (React error #185) in React 19.
 * This uses CSS-based hover tooltip as a workaround.
 */

function TooltipProvider({ children }: { children: React.ReactNode; delayDuration?: number }) {
  return <>{children}</>
}

function Tooltip({ children }: { children: React.ReactNode }) {
  return <div className="group/tooltip relative inline-flex">{children}</div>
}

function TooltipTrigger({
  children,
  asChild,
  ...props
}: React.ComponentProps<"button"> & { asChild?: boolean }) {
  if (asChild && React.isValidElement(children)) {
    return React.cloneElement(children as React.ReactElement<Record<string, unknown>>, {
      ...props,
      "data-slot": "tooltip-trigger",
    })
  }
  return (
    <button data-slot="tooltip-trigger" {...props}>
      {children}
    </button>
  )
}

function TooltipContent({
  className,
  children,
  side = "top",
  ...props
}: React.ComponentProps<"div"> & { side?: "top" | "bottom" | "left" | "right"; sideOffset?: number }) {
  const positionClasses = {
    top: "bottom-full left-1/2 -translate-x-1/2 mb-2",
    bottom: "top-full left-1/2 -translate-x-1/2 mt-2",
    left: "right-full top-1/2 -translate-y-1/2 mr-2",
    right: "left-full top-1/2 -translate-y-1/2 ml-2",
  }

  return (
    <div
      data-slot="tooltip-content"
      className={cn(
        "pointer-events-none absolute z-50 hidden w-max rounded-md bg-foreground px-3 py-1.5 text-xs text-background opacity-0 transition-opacity group-hover/tooltip:block group-hover/tooltip:opacity-100",
        positionClasses[side],
        className
      )}
      {...props}
    >
      {children}
    </div>
  )
}

export { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider }
