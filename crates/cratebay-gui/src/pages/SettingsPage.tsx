import { useSettingsStore } from "@/stores/settingsStore";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { ProviderForm } from "@/components/settings/ProviderForm";
import { ProviderCard } from "@/components/settings/ProviderCard";
import { ReasoningEffort } from "@/components/settings/ReasoningEffort";

export function SettingsPage() {
  return (
    <div className="flex h-full flex-col overflow-auto p-6">
      <h1 className="mb-6 text-xl font-semibold text-foreground">Settings</h1>

      <Tabs defaultValue="providers" className="flex-1">
        <TabsList>
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="providers">LLM Providers</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>

        <TabsContent value="general" className="mt-4">
          <GeneralTab />
        </TabsContent>

        <TabsContent value="providers" className="mt-4">
          <ProvidersTab />
        </TabsContent>

        <TabsContent value="advanced" className="mt-4">
          <AdvancedTab />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function GeneralTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="flex max-w-lg flex-col gap-6">
      {/* Language */}
      <div className="flex flex-col gap-1.5">
        <Label>Language</Label>
        <Select
          value={settings.language}
          onValueChange={(v) => void updateSettings({ language: v as "en" | "zh-CN" })}
        >
          <SelectTrigger className="w-48">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="en">English</SelectItem>
            <SelectItem value="zh-CN">Simplified Chinese</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* Theme */}
      <div className="flex flex-col gap-1.5">
        <Label>Theme</Label>
        <Select
          value={settings.theme}
          onValueChange={(v) =>
            void updateSettings({ theme: v as "dark" | "light" | "system" })
          }
        >
          <SelectTrigger className="w-48">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="dark">Dark</SelectItem>
            <SelectItem value="light">Light</SelectItem>
            <SelectItem value="system">System</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* Send on Enter */}
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-0.5">
          <Label>Send on Enter</Label>
          <p className="text-xs text-muted-foreground">
            Press Enter to send messages, Shift+Enter for new line.
          </p>
        </div>
        <Switch
          checked={settings.sendOnEnter}
          onCheckedChange={(v) => void updateSettings({ sendOnEnter: v })}
        />
      </div>

      {/* Show agent thinking */}
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-0.5">
          <Label>Show Agent Thinking</Label>
          <p className="text-xs text-muted-foreground">
            Display the agent&apos;s reasoning process during responses.
          </p>
        </div>
        <Switch
          checked={settings.showAgentThinking}
          onCheckedChange={(v) => void updateSettings({ showAgentThinking: v })}
        />
      </div>
    </div>
  );
}

function ProvidersTab() {
  const providers = useSettingsStore((s) => s.providers);

  return (
    <div className="flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-medium text-foreground">LLM Providers</h2>
          <p className="text-xs text-muted-foreground">
            Configure API providers for AI models. API keys are encrypted on the backend.
          </p>
        </div>
        <ProviderForm />
      </div>

      {/* Provider list */}
      {providers.length === 0 ? (
        <div className="rounded-lg border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
          No providers configured. Click &quot;Add Provider&quot; to get started.
        </div>
      ) : (
        <div className="grid gap-4">
          {providers.map((provider) => (
            <ProviderCard key={provider.id} provider={provider} />
          ))}
        </div>
      )}

      {/* Reasoning Effort (global setting) */}
      <div className="border-t border-border pt-4">
        <ReasoningEffort />
      </div>
    </div>
  );
}

function AdvancedTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="flex max-w-lg flex-col gap-6">
      {/* Container default TTL */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="ttl">Container Default TTL (hours)</Label>
        <Input
          id="ttl"
          type="number"
          min={1}
          max={168}
          value={settings.containerDefaultTtlHours}
          onChange={(e) =>
            void updateSettings({
              containerDefaultTtlHours: Number(e.target.value) || 8,
            })
          }
          className="w-32"
        />
        <p className="text-xs text-muted-foreground">
          Default time-to-live for new containers (1-168 hours).
        </p>
      </div>

      {/* Max conversation history */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="history">Max Conversation History</Label>
        <Input
          id="history"
          type="number"
          min={10}
          max={200}
          value={settings.maxConversationHistory}
          onChange={(e) =>
            void updateSettings({
              maxConversationHistory: Number(e.target.value) || 50,
            })
          }
          className="w-32"
        />
        <p className="text-xs text-muted-foreground">
          Maximum number of messages to keep in context.
        </p>
      </div>

      {/* Confirm destructive operations */}
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-0.5">
          <Label>Confirm Destructive Operations</Label>
          <p className="text-xs text-muted-foreground">
            Show a confirmation dialog before delete/stop operations.
          </p>
        </div>
        <Switch
          checked={settings.confirmDestructiveOps}
          onCheckedChange={(v) => void updateSettings({ confirmDestructiveOps: v })}
        />
      </div>
    </div>
  );
}
