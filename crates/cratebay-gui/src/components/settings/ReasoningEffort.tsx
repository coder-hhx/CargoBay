import { useSettingsStore } from "@/stores/settingsStore";
import { cn } from "@/lib/utils";

type ReasoningLevel = "low" | "medium" | "high";

const LEVELS: { value: ReasoningLevel; label: string }[] = [
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

export function ReasoningEffort() {
  const reasoningEffort = useSettingsStore((s) => s.settings.reasoningEffort);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-sm font-medium text-foreground">Reasoning Effort</span>
      <p className="text-xs text-muted-foreground">
        Controls how much reasoning the model performs. Only applies when using OpenAI Responses API
        format.
      </p>
      <div className="mt-1 inline-flex rounded-md border border-border bg-muted p-0.5">
        {LEVELS.map((level) => (
          <button
            key={level.value}
            onClick={() => void updateSettings({ reasoningEffort: level.value })}
            className={cn(
              "rounded-sm px-3 py-1 text-xs font-medium transition-colors",
              reasoningEffort === level.value
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {level.label}
          </button>
        ))}
      </div>
    </div>
  );
}
