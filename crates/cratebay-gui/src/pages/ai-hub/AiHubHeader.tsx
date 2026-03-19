import { I } from "../../icons"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent } from "@/components/ui/card"

interface AiHubHeaderProps {
  t: (key: string) => string
  tabLabel: string
}

export function AiHubHeader({ t, tabLabel }: AiHubHeaderProps) {
  return (
    <Card className="py-0">
      <CardContent className="space-y-3 py-4">
        <div className="flex items-start gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary [&_svg]:size-5 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
            {I.aiAssistant}
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <div className="text-sm font-semibold text-foreground">{t("ai")}</div>
              <Badge
                variant="secondary"
                className="rounded-md border border-brand-cyan/15 bg-brand-cyan/10 px-1.5 py-0 text-[11px] text-brand-cyan"
              >
                {t("aiInfra")}
              </Badge>
              <span className="text-muted-foreground/40">•</span>
              <span className="text-xs text-muted-foreground">
                {t("aiHubActiveTab")}: <span className="font-medium text-foreground/90">{tabLabel}</span>
              </span>
            </div>
            <div className="mt-1 text-xs text-muted-foreground">{t("aiHubDesc")}</div>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
