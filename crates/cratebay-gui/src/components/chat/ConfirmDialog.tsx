import { useState } from "react";
import { cn } from "@/lib/utils";
import type { ConfirmationRequest } from "@/stores/workflowStore";
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
import { AlertTriangle, ShieldAlert } from "lucide-react";

interface ConfirmDialogProps {
  request: ConfirmationRequest;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Modal dialog for destructive operations requiring user approval.
 *
 * Risk Level Styling:
 * - medium: Warning (amber), single click confirm
 * - high: Destructive (red), type confirmation text
 * - critical: Destructive + bold, type exact resource name
 */
export function ConfirmDialog({ request, onConfirm, onCancel }: ConfirmDialogProps) {
  const [confirmText, setConfirmText] = useState("");

  const needsTextConfirmation = request.riskLevel === "high" || request.riskLevel === "critical";
  const expectedText = request.riskLevel === "critical" ? request.toolName : "confirm";
  const canConfirm = needsTextConfirmation ? confirmText === expectedText : true;

  const riskConfig = getRiskConfig(request.riskLevel);

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onCancel(); }}>
      <DialogContent className="sm:max-w-[425px]">
        <DialogHeader>
          <div className="flex items-center gap-2">
            <riskConfig.Icon className={cn("h-5 w-5", riskConfig.iconClass)} />
            <DialogTitle className={riskConfig.titleClass}>
              Confirm {request.toolLabel}
            </DialogTitle>
          </div>
          <DialogDescription>{request.description}</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          {/* Parameter summary */}
          <div className="rounded-md bg-muted/50 p-3">
            <p className="mb-1 text-xs font-medium text-muted-foreground">Parameters:</p>
            <pre className="overflow-x-auto font-mono text-xs text-muted-foreground">
              {JSON.stringify(request.parameters, null, 2)}
            </pre>
          </div>

          {/* Consequences list */}
          {request.consequences.length > 0 && (
            <div>
              <p className="mb-1 text-xs font-medium text-muted-foreground">Consequences:</p>
              <ul className="list-inside list-disc space-y-0.5 text-xs text-muted-foreground">
                {request.consequences.map((c, i) => (
                  <li key={i}>{c}</li>
                ))}
              </ul>
            </div>
          )}

          {/* Text confirmation input */}
          {needsTextConfirmation && (
            <div>
              <p className="mb-1 text-xs text-muted-foreground">
                Type <code className="rounded bg-muted px-1 font-mono">{expectedText}</code> to
                confirm:
              </p>
              <Input
                value={confirmText}
                onChange={(e) => setConfirmText(e.target.value)}
                placeholder={expectedText}
                className="text-sm"
                autoFocus
              />
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button
            variant={riskConfig.buttonVariant}
            onClick={onConfirm}
            disabled={!canConfirm}
          >
            Confirm
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface RiskConfig {
  Icon: React.ComponentType<{ className?: string }>;
  iconClass: string;
  titleClass: string;
  buttonVariant: "default" | "destructive";
}

function getRiskConfig(riskLevel: ConfirmationRequest["riskLevel"]): RiskConfig {
  switch (riskLevel) {
    case "medium":
      return {
        Icon: AlertTriangle,
        iconClass: "text-yellow-500",
        titleClass: "text-foreground",
        buttonVariant: "default",
      };
    case "high":
      return {
        Icon: ShieldAlert,
        iconClass: "text-destructive",
        titleClass: "text-destructive",
        buttonVariant: "destructive",
      };
    case "critical":
      return {
        Icon: ShieldAlert,
        iconClass: "text-destructive",
        titleClass: "text-destructive font-bold",
        buttonVariant: "destructive",
      };
  }
}
