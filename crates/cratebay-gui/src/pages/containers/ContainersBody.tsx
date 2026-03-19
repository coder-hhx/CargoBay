import type { ReactNode } from "react"
import { I } from "../../icons"
import { EmptyState } from "../../components/EmptyState"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { cn } from "@/lib/utils"
import { iconStroke } from "@/lib/styles"
import type { ContainerGroup, ContainerInfo } from "../../types"

interface ContainersBodyProps {
  t: (key: string) => string
  creating: boolean
  createFailed: string
  createImageName: string
  createStatus: string
  groups: ContainerGroup[]
  expandedGroups: Record<string, boolean>
  onFetch: () => void
  onOpenRunModal: () => void
  onToggleGroup: (key: string) => void
  onDismissCreateError: () => void
  renderCard: (container: ContainerInfo, opts?: { child?: boolean }) => ReactNode
  spinner: ReactNode
}

export function ContainersBody({
  t,
  creating,
  createFailed,
  createImageName,
  createStatus,
  groups,
  expandedGroups,
  onFetch,
  onOpenRunModal,
  onToggleGroup,
  onDismissCreateError,
  renderCard,
  spinner,
}: ContainersBodyProps) {
  return (
    <>
      <div className="flex items-center justify-end gap-2">
        <Button type="button" variant="outline" size="sm" onClick={onFetch}>
          <span className={cn(iconStroke, "[&_svg]:size-4")}>{I.refresh}</span>
          {t("refresh")}
        </Button>
        <Button type="button" onClick={onOpenRunModal} data-testid="containers-run">
          <span className={cn(iconStroke, "[&_svg]:size-4")}>{I.plus}</span>
          {t("runNewContainer")}
        </Button>
      </div>

      <div className="space-y-3">
        {creating && (
          <Card className={cn("py-0", createFailed && "border-destructive/30")}>
            <CardContent className="flex items-start justify-between gap-3 py-3">
              <div className="flex items-start gap-3">
                <div
                  className={cn(
                    "mt-0.5 flex size-9 items-center justify-center rounded-lg",
                    createFailed ? "bg-destructive/10 text-destructive" : "bg-primary/10 text-primary",
                  )}
                >
                  {createFailed ? (
                    <span className={cn(iconStroke, "[&_svg]:size-4")}>{I.alertCircle}</span>
                  ) : (
                    spinner
                  )}
                </div>

                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold text-foreground">{createImageName}</div>
                  <div
                    className={cn(
                      "mt-0.5 whitespace-pre-wrap text-xs",
                      createFailed ? "text-destructive/90" : "text-muted-foreground",
                    )}
                  >
                    {createStatus}
                  </div>
                  {createFailed && (
                    <div className="mt-2 whitespace-pre-wrap text-xs text-muted-foreground">{createFailed}</div>
                  )}
                </div>
              </div>

              {createFailed && (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  className="hover:bg-destructive/10 hover:text-destructive"
                  onClick={onDismissCreateError}
                  aria-label={t("close")}
                >
                  ×
                </Button>
              )}
            </CardContent>
          </Card>
        )}

        {groups.length === 0 && !creating ? (
          <EmptyState
            icon={I.box}
            title={t("noContainers")}
            description={t("runContainerTip")}
            code="docker run -it -p 80:80 docker/getting-started"
          />
        ) : (
          groups.map((group) => {
            if (group.containers.length <= 1) {
              return renderCard(group.containers[0])
            }

            const expanded = !!expandedGroups[group.key]
            return (
              <Collapsible key={group.key} open={expanded} onOpenChange={() => onToggleGroup(group.key)}>
                <Card className="gap-0 py-0">
                  <CardContent className="px-0">
                    <CollapsibleTrigger asChild>
                      <button
                        type="button"
                        title={expanded ? "Collapse" : "Expand"}
                        className="flex w-full items-center justify-between gap-3 rounded-xl px-4 py-3 text-left transition-colors hover:bg-accent/30"
                      >
                        <div className="flex min-w-0 items-start gap-3">
                          <div className="mt-0.5 flex size-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
                            <span className={cn(iconStroke, "[&_svg]:size-4")}>{I.layers}</span>
                          </div>
                          <div className="min-w-0">
                            <div className="truncate text-sm font-semibold text-foreground">{group.key}</div>
                            <div className="mt-0.5 text-xs text-muted-foreground">
                              {group.containers.length} {t("containers")}
                            </div>
                          </div>
                        </div>

                        <div className="flex items-center gap-3">
                          <Badge
                            variant="secondary"
                            className={cn(
                              "rounded-full border border-border/60 bg-popover/40 px-3 py-1 text-xs font-medium",
                              group.runningCount > 0 ? "text-brand-green" : "text-muted-foreground",
                            )}
                          >
                            <span
                              className={cn(
                                "size-1.5 rounded-full",
                                group.runningCount > 0 ? "bg-brand-green" : "bg-muted-foreground/70",
                              )}
                            />
                            {group.runningCount} {t("running")}
                          </Badge>

                          {group.stoppedCount > 0 && (
                            <Badge
                              variant="secondary"
                              className="rounded-full border border-border/60 bg-popover/40 px-3 py-1 text-xs font-medium text-muted-foreground"
                            >
                              <span className="size-1.5 rounded-full bg-muted-foreground/70" />
                              {group.stoppedCount} {t("stopped")}
                            </Badge>
                          )}

                          <span className={cn(iconStroke, "text-muted-foreground [&_svg]:size-4")}>
                            {expanded ? I.chevronDown : I.chevronRight}
                          </span>
                        </div>
                      </button>
                    </CollapsibleTrigger>
                  </CardContent>
                </Card>

                <CollapsibleContent className="mt-2 space-y-3">
                  {group.containers.map((container) => renderCard(container, { child: true }))}
                </CollapsibleContent>
              </Collapsible>
            )
          })
        )}
      </div>
    </>
  )
}
