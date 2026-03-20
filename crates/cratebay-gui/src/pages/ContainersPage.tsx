import { useEffect } from "react";
import { useContainerStore } from "@/stores/containerStore";
import { ContainerList } from "@/components/container/ContainerList";
import { ContainerDetail } from "@/components/container/ContainerDetail";
import { ContainerCreate } from "@/components/container/ContainerCreate";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { RefreshCw } from "lucide-react";

export function ContainersPage() {
  const fetchContainers = useContainerStore((s) => s.fetchContainers);
  const fetchTemplates = useContainerStore((s) => s.fetchTemplates);
  const filter = useContainerStore((s) => s.filter);
  const setFilter = useContainerStore((s) => s.setFilter);
  const loading = useContainerStore((s) => s.loading);

  useEffect(() => {
    void fetchContainers();
    void fetchTemplates();
  }, [fetchContainers, fetchTemplates]);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-6 py-4">
        <div>
          <h1 className="text-lg font-semibold text-foreground">Containers</h1>
          <p className="text-xs text-muted-foreground">
            Manage development sandboxes and containers.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void fetchContainers()}
            disabled={loading}
          >
            <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
            Refresh
          </Button>
          <ContainerCreate />
        </div>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3 border-b border-border px-6 py-3">
        <Input
          value={filter.search}
          onChange={(e) => setFilter({ search: e.target.value })}
          placeholder="Search containers..."
          className="max-w-xs"
        />
        <Select
          value={filter.status}
          onValueChange={(v) => setFilter({ status: v as "all" | "running" | "stopped" })}
        >
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All Statuses</SelectItem>
            <SelectItem value="running">Running</SelectItem>
            <SelectItem value="stopped">Stopped</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* Container table */}
      <div className="flex-1 overflow-auto">
        <ContainerList />
      </div>

      {/* Detail dialog */}
      <ContainerDetail />
    </div>
  );
}
