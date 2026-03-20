import { useSettingsStore, type LlmModelInfo } from "@/stores/settingsStore";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";

interface ModelSelectorProps {
  providerId: string;
  models: LlmModelInfo[];
}

export function ModelSelector({ providerId, models }: ModelSelectorProps) {
  const toggleModel = useSettingsStore((s) => s.toggleModel);

  if (models.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">
        No models loaded. Click &quot;Fetch Models&quot; to load available models.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {models.map((model) => (
        <div key={model.id} className="flex items-center gap-2">
          <Checkbox
            id={`model-${model.id}`}
            checked={model.isEnabled}
            onCheckedChange={(checked) => {
              void toggleModel(providerId, model.id, checked === true);
            }}
          />
          <Label
            htmlFor={`model-${model.id}`}
            className="cursor-pointer text-sm font-normal text-foreground"
          >
            {model.name || model.id}
          </Label>
        </div>
      ))}
    </div>
  );
}
