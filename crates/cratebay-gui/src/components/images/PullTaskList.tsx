import { usePullStore } from "@/stores/pullStore";
import type { PullTask } from "@/stores/pullStore";
import { Button } from "@/components/ui/button";
import { X, CheckCircle2, XCircle, Download, Loader2 } from "lucide-react";

export function PullTaskList() {
  const tasks = usePullStore((s) => s.tasks);
  const removeTask = usePullStore((s) => s.removeTask);
  const clearCompleted = usePullStore((s) => s.clearCompleted);

  if (tasks.length === 0) return null;

  const completedCount = tasks.filter((t) => t.complete).length;

  return (
    <div className="mx-6 mt-3 rounded-lg border border-border bg-card">
      {/* 标题栏 */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <div className="flex items-center gap-2 text-sm font-medium text-foreground">
          <Download className="h-3.5 w-3.5 text-primary" />
          <span>拉取任务 ({tasks.length})</span>
        </div>
        {completedCount > 0 && (
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2 text-xs text-muted-foreground"
            onClick={clearCompleted}
          >
            清除已完成
          </Button>
        )}
      </div>

      {/* 任务列表 */}
      <div className="divide-y divide-border">
        {tasks.map((task) => (
          <PullTaskRow key={task.id} task={task} onRemove={() => removeTask(task.id)} />
        ))}
      </div>
    </div>
  );
}

function PullTaskRow({ task, onRemove }: { task: PullTask; onRemove: () => void }) {
  if (task.complete) {
    return (
      <div className="flex items-center justify-between gap-3 px-4 py-2">
        <div className="flex items-center gap-2 min-w-0">
          {task.error !== null ? (
            <XCircle className="h-3.5 w-3.5 flex-shrink-0 text-destructive" />
          ) : (
            <CheckCircle2 className="h-3.5 w-3.5 flex-shrink-0 text-green-500" />
          )}
          <span className="truncate text-sm font-mono text-foreground">{task.image}</span>
          {task.error !== null ? (
            <span className="truncate text-xs text-destructive">{task.error}</span>
          ) : (
            <span className="text-xs text-muted-foreground">已完成</span>
          )}
        </div>
        <button
          onClick={onRemove}
          className="flex-shrink-0 rounded p-0.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between gap-3 px-4 py-2">
      <div className="flex items-center gap-2 min-w-0 flex-1">
        <Loader2 className="h-3.5 w-3.5 flex-shrink-0 animate-spin text-primary" />
        <span className="truncate text-sm font-mono text-foreground">{task.image}</span>
      </div>
      <div className="flex items-center gap-2 flex-shrink-0">
        <div className="h-1.5 w-20 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full bg-primary transition-all duration-300"
            style={{ width: `${Math.max(task.progress, 2)}%` }}
          />
        </div>
        <span className="text-[10px] tabular-nums text-muted-foreground w-8 text-right">
          {task.progress}%
        </span>
      </div>
    </div>
  );
}
