import { useState } from "react";
import { useI18n } from "@/lib/i18n";
import type { McpServerConfig } from "@/types/mcp";
import type { McpServerInfo } from "@/types/mcp";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface McpServerConfigProps {
  /** If provided, we're editing an existing server; otherwise adding a new one. */
  server?: McpServerInfo;
  open: boolean;
  onClose: () => void;
  onSave: (config: McpServerConfig) => void;
}

/**
 * Server add/edit form dialog.
 */
export function McpServerConfigDialog({
  server,
  open,
  onClose,
  onSave,
}: McpServerConfigProps) {
  const { t } = useI18n();
  const isEditing = server !== undefined;

  const [name, setName] = useState(server?.name ?? "");
  const [command, setCommand] = useState(server?.command ?? "");
  const [argsStr, setArgsStr] = useState(server?.args.join(" ") ?? "");
  const [envStr, setEnvStr] = useState(
    server !== undefined
      ? Object.entries(server.env)
          .map(([k, v]) => `${k}=${v}`)
          .join("\n")
      : "",
  );

  const canSave = name.trim().length > 0 && command.trim().length > 0;

  const handleSave = () => {
    if (!canSave) return;

    const args = argsStr
      .trim()
      .split(/\s+/)
      .filter((a) => a.length > 0);

    const env: Record<string, string> = {};
    for (const line of envStr.split("\n")) {
      const trimmed = line.trim();
      if (trimmed.length === 0) continue;
      const eqIdx = trimmed.indexOf("=");
      if (eqIdx > 0) {
        env[trimmed.slice(0, eqIdx)] = trimmed.slice(eqIdx + 1);
      }
    }

    onSave({
      name: name.trim(),
      command: command.trim(),
      args,
      env: Object.keys(env).length > 0 ? env : undefined,
    });
  };

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="sm:max-w-[450px]">
        <DialogHeader>
          <DialogTitle>{isEditing ? t("mcp", "editServerTitle") : t("mcp", "addServerTitle")}</DialogTitle>
          <DialogDescription>
            {isEditing
              ? t("mcp", "editServerDesc")
              : t("mcp", "addServerDesc")}
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4 py-2">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="mcp-name">{t("mcp", "name")}</Label>
            <Input
              id="mcp-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., My MCP Server"
            />
          </div>

          <div className="flex flex-col gap-1.5">
            <Label htmlFor="mcp-command">{t("mcp", "command")}</Label>
            <Input
              id="mcp-command"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder="e.g., npx -y @modelcontextprotocol/server"
              className="font-mono text-sm"
            />
          </div>

          <div className="flex flex-col gap-1.5">
            <Label htmlFor="mcp-args">{t("mcp", "arguments")}</Label>
            <Input
              id="mcp-args"
              value={argsStr}
              onChange={(e) => setArgsStr(e.target.value)}
              placeholder="e.g., --port 3000"
              className="font-mono text-sm"
            />
          </div>

          <div className="flex flex-col gap-1.5">
            <Label htmlFor="mcp-env">{t("mcp", "envVars")}</Label>
            <textarea
              id="mcp-env"
              value={envStr}
              onChange={(e) => setEnvStr(e.target.value)}
              placeholder={"API_KEY=abc123\nDEBUG=true"}
              className="min-h-[80px] w-full resize-none rounded-md border border-border bg-transparent px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
              rows={3}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("common", "cancel")}
          </Button>
          <Button onClick={handleSave} disabled={!canSave}>
            {isEditing ? t("mcp", "saveChanges") : t("mcp", "addServer")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
