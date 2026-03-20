import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { PanelLeftClose, PanelLeftOpen } from "lucide-react";

const pageLabels: Record<string, string> = {
  chat: "Chat",
  containers: "Containers",
  mcp: "MCP Servers",
  settings: "Settings",
};

export function TopBar() {
  const currentPage = useAppStore((s) => s.currentPage);
  const sidebarOpen = useAppStore((s) => s.sidebarOpen);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);

  return (
    <header className="flex h-12 flex-shrink-0 items-center gap-3 border-b border-border px-4">
      {/* Sidebar toggle */}
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={toggleSidebar}
        aria-label={sidebarOpen ? "Collapse sidebar" : "Expand sidebar"}
      >
        {sidebarOpen ? (
          <PanelLeftClose className="h-4 w-4" />
        ) : (
          <PanelLeftOpen className="h-4 w-4" />
        )}
      </Button>

      {/* Breadcrumb */}
      <div className="flex items-center gap-1.5 text-sm">
        <span className="text-muted-foreground">CrateBay</span>
        <span className="text-muted-foreground">/</span>
        <span className="font-medium text-foreground">
          {pageLabels[currentPage] ?? currentPage}
        </span>
      </div>
    </header>
  );
}
