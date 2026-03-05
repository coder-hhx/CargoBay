import { useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { langNames } from "../i18n/messages"
import { I } from "../icons"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { cardActionOutline, cardActionSecondary } from "@/lib/styles"
import type {
  AiProfileValidationResult,
  AiProviderProfile,
  AiSettings,
  Theme,
} from "../types"

interface UpdateInfo {
  available: boolean
  current_version: string
  latest_version: string
  release_notes: string
  download_url: string
}

interface SettingsProps {
  theme: Theme
  setTheme: (v: Theme) => void
  lang: string
  setLang: (v: string) => void
  t: (key: string) => string
}


export function Settings({ theme, setTheme, lang, setLang, t }: SettingsProps) {
  const normalizeLang = (value: string) => (value === "zh" ? "zh" : "en")
  const [checking, setChecking] = useState(false)
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null)
  const [updateError, setUpdateError] = useState("")
  const [aiLoading, setAiLoading] = useState(true)
  const [aiSaving, setAiSaving] = useState(false)
  const [aiValidating, setAiValidating] = useState(false)
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null)
  const [aiError, setAiError] = useState("")
  const [aiMessage, setAiMessage] = useState("")
  const [headersJson, setHeadersJson] = useState("{}")

  const sectionTitle = (key: string) => {
    const value = t(key)
    return value.length <= 24 ? value.toUpperCase() : value
  }

  const activeProfile = useMemo(() => {
    if (!aiSettings) return null
    return aiSettings.profiles.find((profile) => profile.id === aiSettings.active_profile_id) ?? null
  }, [aiSettings])

  useEffect(() => {
    const loadAiSettings = async () => {
      setAiLoading(true)
      setAiError("")
      try {
        const settings = await invoke<AiSettings>("load_ai_settings")
        setAiSettings(settings)
      } catch (e) {
        setAiError(String(e))
      } finally {
        setAiLoading(false)
      }
    }
    loadAiSettings()
  }, [])

  useEffect(() => {
    if (!activeProfile) {
      setHeadersJson("{}")
      return
    }
    setHeadersJson(JSON.stringify(activeProfile.headers ?? {}, null, 2))
  }, [activeProfile])

  const handleCheckUpdate = async () => {
    setChecking(true)
    setUpdateError("")
    setUpdateInfo(null)
    try {
      const info = await invoke<UpdateInfo>("check_update")
      setUpdateInfo(info)
    } catch (e) {
      setUpdateError(String(e))
    } finally {
      setChecking(false)
    }
  }

  const handleViewRelease = async () => {
    if (!updateInfo?.download_url) return
    try {
      await invoke("open_release_page", { url: updateInfo.download_url })
    } catch {
      window.open(updateInfo.download_url, "_blank")
    }
  }

  const parseHeadersJson = (): Record<string, string> | null => {
    const raw = headersJson.trim()
    if (!raw) return {}
    try {
      const parsed = JSON.parse(raw) as unknown
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        setAiError(t("aiHeadersJsonError"))
        return null
      }
      const out: Record<string, string> = {}
      for (const [key, value] of Object.entries(parsed)) {
        if (typeof value !== "string") {
          setAiError(t("aiHeadersJsonError"))
          return null
        }
        out[key] = value
      }
      return out
    } catch {
      setAiError(t("aiHeadersJsonError"))
      return null
    }
  }

  const updateActiveProfile = (
    updater: (profile: AiProviderProfile) => AiProviderProfile
  ) => {
    setAiSettings((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        profiles: prev.profiles.map((profile) =>
          profile.id === prev.active_profile_id ? updater(profile) : profile
        ),
      }
    })
  }

  const resolveActiveProfileWithHeaders = (): AiProviderProfile | null => {
    if (!activeProfile) return null
    const headers = parseHeadersJson()
    if (!headers) return null
    return { ...activeProfile, headers }
  }

  const handleAiSaveSettings = async () => {
    if (!aiSettings || !activeProfile) return
    const profile = resolveActiveProfileWithHeaders()
    if (!profile) return

    const nextSettings: AiSettings = {
      ...aiSettings,
      profiles: aiSettings.profiles.map((item) =>
        item.id === aiSettings.active_profile_id ? profile : item
      ),
    }

    setAiSaving(true)
    setAiError("")
    setAiMessage("")
    try {
      const saved = await invoke<AiSettings>("save_ai_settings", { settings: nextSettings })
      setAiSettings(saved)
      setAiMessage(t("aiSettingsSaved"))
    } catch (e) {
      setAiError(String(e))
    } finally {
      setAiSaving(false)
    }
  }

  const handleAiValidateProfile = async () => {
    if (!activeProfile) return
    const profile = resolveActiveProfileWithHeaders()
    if (!profile) return

    setAiValidating(true)
    setAiError("")
    setAiMessage("")
    try {
      const result = await invoke<AiProfileValidationResult>("validate_ai_profile", { profile })
      if (result.ok) {
        setAiMessage(result.message || t("aiValidationPassed"))
      } else {
        setAiError(result.message || t("aiValidationFailed"))
      }
    } catch (e) {
      setAiError(String(e))
    } finally {
      setAiValidating(false)
    }
  }

  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <div className="text-xs font-semibold tracking-widest text-muted-foreground">
          {sectionTitle("theme")}
        </div>
        <Card className="py-0">
          <CardContent className="py-4">
            <div className="flex items-center gap-4">
              <div className="size-10 shrink-0 rounded-lg bg-primary/10 text-primary flex items-center justify-center [&_svg]:size-5 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
                {I.moon}
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold text-foreground">
                  {t("theme")}
                </div>
                <div className="text-xs text-muted-foreground">
                  {t("themeDesc")}
                </div>
              </div>
              <Select
                value={theme}
                onValueChange={(v) => setTheme(v as Theme)}
              >
                <SelectTrigger size="sm" className="w-[140px] justify-between">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent align="end">
                  <SelectItem value="system">{t("systemTheme")}</SelectItem>
                  <SelectItem value="dark">{t("dark")}</SelectItem>
                  <SelectItem value="light">{t("light")}</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </CardContent>
        </Card>
      </section>

      <section className="space-y-3">
        <div className="text-xs font-semibold tracking-widest text-muted-foreground">
          {sectionTitle("language")}
        </div>
        <Card className="py-0">
          <CardContent className="py-4">
            <div className="flex items-center gap-4">
              <div className="size-10 shrink-0 rounded-lg bg-primary/10 text-primary flex items-center justify-center [&_svg]:size-5 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
                {I.globe}
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold text-foreground">
                  {t("language")}
                </div>
                <div className="text-xs text-muted-foreground">
                  {t("languageDesc")}
                </div>
              </div>
              <Select
                value={lang}
                onValueChange={(v) => setLang(normalizeLang(v))}
              >
                <SelectTrigger size="sm" className="w-[140px] justify-between">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent align="end">
                  {Object.entries(langNames).map(([code, name]) => (
                    <SelectItem key={code} value={code}>
                      {name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </CardContent>
        </Card>
      </section>

      <section className="space-y-3">
        <div className="text-xs font-semibold tracking-widest text-muted-foreground">
          {sectionTitle("aiSettings")}
        </div>
        <Card className="py-0">
          <CardContent className="space-y-4 py-4">
            <div className="flex items-center gap-4">
              <div className="size-10 shrink-0 rounded-lg bg-primary/10 text-primary flex items-center justify-center [&_svg]:size-5 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
                {I.key}
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold text-foreground">
                  {t("aiSettings")}
                </div>
                <div className="text-xs text-muted-foreground">
                  {t("aiSettingsDesc")}
                </div>
              </div>
            </div>

            {aiLoading && (
              <div className="text-sm text-muted-foreground">{t("loading")}</div>
            )}

            {!aiLoading && aiSettings && activeProfile && (
              <div className="space-y-4">
                <div className="grid gap-3 md:grid-cols-2">
                  <div>
                    <label className="text-xs font-semibold text-muted-foreground">{t("aiActiveProfile")}</label>
                    <Select
                      value={aiSettings.active_profile_id}
                      onValueChange={(value) => {
                        setAiError("")
                        setAiMessage("")
                        setAiSettings((prev) => (prev ? { ...prev, active_profile_id: value } : prev))
                      }}
                    >
                      <SelectTrigger size="sm" className="mt-1 w-full justify-between">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent align="end">
                        {aiSettings.profiles.map((profile) => (
                          <SelectItem key={profile.id} value={profile.id}>
                            {profile.display_name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div>
                    <label className="text-xs font-semibold text-muted-foreground">{t("aiDisplayName")}</label>
                    <Input
                      className="mt-1"
                      value={activeProfile.display_name}
                      onChange={(event) => updateActiveProfile((profile) => ({
                        ...profile,
                        display_name: event.target.value,
                      }))}
                    />
                  </div>
                  <div>
                    <label className="text-xs font-semibold text-muted-foreground">{t("aiProviderId")}</label>
                    <Input
                      className="mt-1"
                      value={activeProfile.provider_id}
                      onChange={(event) => updateActiveProfile((profile) => ({
                        ...profile,
                        provider_id: event.target.value,
                      }))}
                    />
                  </div>
                  <div>
                    <label className="text-xs font-semibold text-muted-foreground">{t("aiModel")}</label>
                    <Input
                      className="mt-1"
                      value={activeProfile.model}
                      onChange={(event) => updateActiveProfile((profile) => ({
                        ...profile,
                        model: event.target.value,
                      }))}
                    />
                  </div>
                  <div>
                    <label className="text-xs font-semibold text-muted-foreground">{t("aiBaseUrl")}</label>
                    <Input
                      className="mt-1"
                      value={activeProfile.base_url}
                      onChange={(event) => updateActiveProfile((profile) => ({
                        ...profile,
                        base_url: event.target.value,
                      }))}
                    />
                  </div>
                  <div>
                    <label className="text-xs font-semibold text-muted-foreground">{t("aiApiKeyRef")}</label>
                    <Input
                      className="mt-1"
                      value={activeProfile.api_key_ref}
                      onChange={(event) => updateActiveProfile((profile) => ({
                        ...profile,
                        api_key_ref: event.target.value,
                      }))}
                    />
                  </div>
                </div>

                <div>
                  <label className="text-xs font-semibold text-muted-foreground">{t("aiHeadersJson")}</label>
                  <textarea
                    value={headersJson}
                    onChange={(event) => setHeadersJson(event.target.value)}
                    spellCheck={false}
                    className="mt-1 min-h-24 w-full rounded-md border border-border bg-background px-3 py-2 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                  />
                  <div className="mt-1 text-xs text-muted-foreground">{t("aiHeadersHint")}</div>
                </div>

                <div className="grid gap-2 md:grid-cols-2">
                  <label className="flex items-center gap-2 text-sm text-muted-foreground">
                    <Checkbox
                      checked={aiSettings.security_policy.destructive_action_confirmation}
                      onCheckedChange={(value) => {
                        const checked = value === true
                        setAiSettings((prev) =>
                          prev
                            ? {
                                ...prev,
                                security_policy: {
                                  ...prev.security_policy,
                                  destructive_action_confirmation: checked,
                                },
                              }
                            : prev
                        )
                      }}
                    />
                    <span>{t("aiDestructiveConfirm")}</span>
                  </label>
                  <label className="flex items-center gap-2 text-sm text-muted-foreground">
                    <Checkbox
                      checked={aiSettings.security_policy.mcp_remote_enabled}
                      onCheckedChange={(value) => {
                        const checked = value === true
                        setAiSettings((prev) =>
                          prev
                            ? {
                                ...prev,
                                security_policy: {
                                  ...prev.security_policy,
                                  mcp_remote_enabled: checked,
                                },
                              }
                            : prev
                        )
                      }}
                    />
                    <span>{t("aiMcpRemoteEnabled")}</span>
                  </label>
                </div>

                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className={cardActionOutline}
                    onClick={handleAiValidateProfile}
                    disabled={aiValidating || aiSaving}
                  >
                    {aiValidating ? t("working") : t("aiValidateProfile")}
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    className={cardActionSecondary}
                    onClick={handleAiSaveSettings}
                    disabled={aiSaving || aiValidating}
                  >
                    {aiSaving ? t("working") : t("aiSaveSettings")}
                  </Button>
                </div>
              </div>
            )}

            {aiError && (
              <Alert variant="destructive">
                <div className="flex items-start gap-3">
                  <div className="mt-0.5 [&_svg]:size-4 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
                    {I.alertCircle}
                  </div>
                  <div className="min-w-0 flex-1">
                    <AlertTitle>{t("aiSettings")}</AlertTitle>
                    <AlertDescription>
                      <p className="whitespace-pre-wrap">{aiError}</p>
                    </AlertDescription>
                  </div>
                </div>
              </Alert>
            )}

            {aiMessage && (
              <Alert className="border-border/70 bg-card">
                <div className="flex items-start gap-3">
                  <div className="mt-0.5 text-primary [&_svg]:size-4 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
                    {I.check}
                  </div>
                  <div className="min-w-0 flex-1">
                    <AlertTitle>{t("aiSettings")}</AlertTitle>
                    <AlertDescription>
                      <p className="whitespace-pre-wrap">{aiMessage}</p>
                    </AlertDescription>
                  </div>
                </div>
              </Alert>
            )}
          </CardContent>
        </Card>
      </section>

      <section className="space-y-3">
        <div className="text-xs font-semibold tracking-widest text-muted-foreground">
          {sectionTitle("updates")}
        </div>
        <Card className="py-0">
          <CardContent className="py-4">
            <div className="flex items-center gap-4">
              <div className="size-10 shrink-0 rounded-lg bg-primary/10 text-primary flex items-center justify-center [&_svg]:size-5 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
                {I.refresh}
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold text-foreground">
                  {t("updates")}
                </div>
                <div className="text-xs text-muted-foreground">
                  {t("currentVersion")}:{" "}
                  <Badge variant="secondary" className="ml-1 rounded-md px-1.5 py-0 text-[10px]">
                    v{updateInfo?.current_version ?? "1.0.0"}
                  </Badge>
                </div>
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className={cardActionOutline}
                onClick={handleCheckUpdate}
                disabled={checking}
              >
                {checking ? t("checkingUpdates") : t("checkUpdates")}
              </Button>
            </div>
          </CardContent>
        </Card>

        {updateInfo && (
          <Alert className="border-border/70 bg-card">
            <div className="flex items-start gap-3">
              <div className="mt-0.5 text-primary [&_svg]:size-4 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
                {updateInfo.available ? I.alertCircle : I.check}
              </div>
              <div className="min-w-0 flex-1">
                <AlertTitle>
                  {updateInfo.available
                    ? `${t("updateAvailable")}: v${updateInfo.latest_version}`
                    : t("noUpdates")}
                </AlertTitle>
                {updateInfo.available && updateInfo.release_notes && (
                  <AlertDescription>
                    <p className="whitespace-pre-wrap">
                      {updateInfo.release_notes}
                    </p>
                  </AlertDescription>
                )}
              </div>
              {updateInfo.available && (
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  className={cardActionSecondary}
                  onClick={handleViewRelease}
                >
                  {t("viewRelease")}
                </Button>
              )}
            </div>
          </Alert>
        )}

        {updateError && (
          <Alert variant="destructive">
            <div className="flex items-start gap-3">
              <div className="mt-0.5 [&_svg]:size-4 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
                {I.alertCircle}
              </div>
              <div className="min-w-0 flex-1">
                <AlertTitle>{t("updates")}</AlertTitle>
                <AlertDescription>
                  <p className="whitespace-pre-wrap">{updateError}</p>
                </AlertDescription>
              </div>
            </div>
          </Alert>
        )}
      </section>
    </div>
  )
}
