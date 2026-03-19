import { I } from "../../icons"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent } from "@/components/ui/card"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { iconStroke } from "@/lib/styles"
import type { SettingsTab } from "../Settings"

interface SettingsHeaderProps {
  t: (key: string) => string
  settingsTab: SettingsTab
  settingsTabLabel: string
  setSettingsTab: (value: SettingsTab) => void
}

export function SettingsHeader({
  t,
  settingsTab,
  settingsTabLabel,
  setSettingsTab,
}: SettingsHeaderProps) {
  return (
    <Card className="py-0">
      <CardContent className="flex flex-wrap items-center justify-between gap-4 py-4">
        <div className="flex items-start gap-3">
          <div className={`flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary [&_svg]:size-5 ${iconStroke}`}>
            {I.settings}
          </div>
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <div className="text-sm font-semibold text-foreground">{t("settings")}</div>
              <Badge variant="secondary" className="rounded-md px-1.5 py-0 text-[11px]">
                {settingsTabLabel}
              </Badge>
            </div>
            <div className="mt-1 text-xs text-muted-foreground">
              {t("settingsGeneralTab")} · {t("settingsAiTab")}
            </div>
          </div>
        </div>

        <Tabs value={settingsTab} onValueChange={(value) => setSettingsTab(value as SettingsTab)}>
          <TabsList className="w-fit">
            <TabsTrigger
              value="general"
              data-testid="settings-tab-general"
              className="flex-none gap-2 px-3"
            >
              <span className={`[&_svg]:size-4 ${iconStroke}`}>{I.settings}</span>
              {t("settingsGeneralTab")}
            </TabsTrigger>
            <TabsTrigger
              value="ai"
              data-testid="settings-tab-ai"
              className="flex-none gap-2 px-3"
            >
              <span className={`[&_svg]:size-4 ${iconStroke}`}>{I.aiAssistant}</span>
              {t("settingsAiTab")}
            </TabsTrigger>
          </TabsList>
        </Tabs>
      </CardContent>
    </Card>
  )
}
