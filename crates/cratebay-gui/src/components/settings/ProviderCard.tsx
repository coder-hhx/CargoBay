import { useCallback, useState } from "react";
import {
  useSettingsStore,
  type LlmProviderInfo,
  type LlmProviderUpdateRequest,
  type ApiFormat,
  type ProviderTestResult,
} from "@/stores/settingsStore";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ModelSelector } from "./ModelSelector";
import {
  Pencil,
  Trash2,
  RefreshCw,
  Eye,
  EyeOff,
  CheckCircle,
  XCircle,
} from "lucide-react";

/** Stable empty references to avoid re-renders from Zustand selectors */
const EMPTY_MODELS: never[] = [];
const MODELS_NOT_LOADING = false;

const API_FORMAT_LABELS: Record<ApiFormat, string> = {
  anthropic: "Anthropic",
  openai_responses: "OpenAI Responses",
  openai_completions: "OpenAI Completions",
};

interface ProviderCardProps {
  provider: LlmProviderInfo;
}

export function ProviderCard({ provider }: ProviderCardProps) {
  const updateProvider = useSettingsStore((s) => s.updateProvider);
  const deleteProvider = useSettingsStore((s) => s.deleteProvider);
  const testProvider = useSettingsStore((s) => s.testProvider);
  const fetchModels = useSettingsStore((s) => s.fetchModels);
  const models = useSettingsStore((s) => s.models[provider.id] ?? EMPTY_MODELS);
  const modelsLoading = useSettingsStore((s) => s.modelsLoading[provider.id] ?? MODELS_NOT_LOADING);

  const [editOpen, setEditOpen] = useState(false);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ProviderTestResult | null>(null);

  const handleTest = useCallback(async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await testProvider(provider.id);
      setTestResult(result);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setTestResult({
        success: false,
        latencyMs: 0,
        model: "",
        error: message,
      });
    } finally {
      setTesting(false);
    }
  }, [testProvider, provider.id]);

  const handleDelete = useCallback(async () => {
    try {
      await deleteProvider(provider.id);
      setDeleteConfirmOpen(false);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setTestResult({
        success: false,
        latencyMs: 0,
        model: "",
        error: message,
      });
    }
  }, [deleteProvider, provider.id]);

  const handleFetchModels = useCallback(async () => {
    try {
      await fetchModels(provider.id);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setTestResult({
        success: false,
        latencyMs: 0,
        model: "",
        error: message,
      });
    }
  }, [fetchModels, provider.id]);

  return (
    <>
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle className="text-base">{provider.name}</CardTitle>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => setEditOpen(true)}
                aria-label="Edit provider"
              >
                <Pencil className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => setDeleteConfirmOpen(true)}
                aria-label="Delete provider"
              >
                <Trash2 className="h-3.5 w-3.5 text-destructive" />
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {/* Provider info */}
          <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <Badge variant="outline" className="text-[10px]">
              {API_FORMAT_LABELS[provider.apiFormat]}
            </Badge>
            <span className="truncate">{provider.apiBase}</span>
            {provider.hasApiKey && (
              <Badge variant="secondary" className="text-[10px]">
                Key set
              </Badge>
            )}
          </div>

          {/* Actions row */}
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="xs"
              onClick={handleTest}
              disabled={testing}
            >
              {testing ? "Testing..." : "Test Connection"}
            </Button>
            {testResult !== null && (
              <span className="flex items-center gap-1 text-xs">
                {testResult.success ? (
                  <>
                    <CheckCircle className="h-3.5 w-3.5 text-success" />
                    <span className="text-success">
                      Connected
                      {testResult.latencyMs > 0 && ` · ${testResult.latencyMs}ms`}
                    </span>
                  </>
                ) : (
                  <>
                    <XCircle className="h-3.5 w-3.5 text-destructive" />
                    <span className="max-w-[280px] truncate text-destructive" title={testResult.error ?? "Failed"}>
                      {testResult.error ?? "Failed"}
                    </span>
                  </>
                )}
              </span>
            )}
          </div>

          {/* Models section */}
          <div className="border-t border-border pt-3">
            <div className="mb-2 flex items-center justify-between">
              <span className="text-sm font-medium text-foreground">Models</span>
              <Button
                variant="outline"
                size="xs"
                onClick={handleFetchModels}
                disabled={modelsLoading}
              >
                <RefreshCw className={`h-3 w-3 ${modelsLoading ? "animate-spin" : ""}`} />
                {modelsLoading ? "Loading..." : "Fetch Models"}
              </Button>
            </div>
            <ModelSelector providerId={provider.id} models={models} />
          </div>
        </CardContent>
      </Card>

      {/* Edit dialog */}
      <EditProviderDialog
        provider={provider}
        open={editOpen}
        onOpenChange={setEditOpen}
        onSave={updateProvider}
      />

      {/* Delete confirmation */}
      <Dialog open={deleteConfirmOpen} onOpenChange={setDeleteConfirmOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Delete Provider</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete &quot;{provider.name}&quot;? This action cannot be
              undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteConfirmOpen(false)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={handleDelete}>
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

function EditProviderDialog({
  provider,
  open,
  onOpenChange,
  onSave,
}: {
  provider: LlmProviderInfo;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: (id: string, request: LlmProviderUpdateRequest) => Promise<LlmProviderInfo>;
}) {
  const [name, setName] = useState(provider.name);
  const [apiBase, setApiBase] = useState(provider.apiBase);
  const [apiKey, setApiKey] = useState("");
  const [apiFormat, setApiFormat] = useState<ApiFormat>(provider.apiFormat);
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);

  const canSave = name.trim().length > 0 && apiBase.trim().length > 0;

  const handleSave = useCallback(async () => {
    if (!canSave) return;
    setSaving(true);
    try {
      const request: LlmProviderUpdateRequest = {
        name: name.trim(),
        apiBase: apiBase.trim(),
        apiFormat,
      };
      if (apiKey.length > 0) {
        request.apiKey = apiKey;
      }
      await onSave(provider.id, request);
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  }, [canSave, name, apiBase, apiKey, apiFormat, onSave, provider.id, onOpenChange]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit Provider</DialogTitle>
          <DialogDescription>
            Update the provider configuration.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4 py-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="edit-name">Provider Name</Label>
            <Input
              id="edit-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="edit-url">Base URL</Label>
            <Input
              id="edit-url"
              value={apiBase}
              onChange={(e) => setApiBase(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="edit-key">API Key (leave empty to keep current)</Label>
            <div className="relative">
              <Input
                id="edit-key"
                type={showKey ? "text" : "password"}
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="Enter new key or leave empty"
                className="pr-10"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                className="absolute right-2 top-1/2 -translate-y-1/2"
                onClick={() => setShowKey(!showKey)}
                aria-label={showKey ? "Hide API key" : "Show API key"}
              >
                {showKey ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
              </Button>
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <Label>API Format</Label>
            <Select value={apiFormat} onValueChange={(v) => setApiFormat(v as ApiFormat)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="anthropic">Anthropic Messages</SelectItem>
                <SelectItem value="openai_responses">OpenAI Responses</SelectItem>
                <SelectItem value="openai_completions">OpenAI Chat Completions</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!canSave || saving}>
            {saving ? "Saving..." : "Save Changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
