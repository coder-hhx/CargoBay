import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { APP_NAME } from "@/lib/constants";
import {
  MessageSquare,
  Box,
  Plug,
  Settings,
  type LucideIcon,
} from "lucide-react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

type PageId = "chat" | "containers" | "mcp" | "settings";

interface NavItem {
  id: PageId;
  label: string;
  icon: LucideIcon;
}

const navItems: NavItem[] = [
  { id: "chat", label: "Chat", icon: MessageSquare },
  { id: "containers", label: "Containers", icon: Box },
  { id: "mcp", label: "MCP", icon: Plug },
  { id: "settings", label: "Settings", icon: Settings },
];

export function Sidebar() {
  const currentPage = useAppStore((s) => s.currentPage);
  const setCurrentPage = useAppStore((s) => s.setCurrentPage);

  return (
    <div className="flex h-full w-full flex-col bg-card">
      {/* Logo & title */}
      <div className="flex h-12 items-center gap-2 px-4">
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-primary text-xs font-bold text-background">
          CB
        </div>
        <span className="truncate text-sm font-semibold text-foreground">
          {APP_NAME}
        </span>
      </div>

      <div className="mx-3 mb-2">
        <div className="h-px bg-border" />
      </div>

      {/* Navigation items */}
      <nav className="flex flex-1 flex-col gap-1 px-3">
        {navItems.map((item) => (
          <SidebarNavItem
            key={item.id}
            item={item}
            active={currentPage === item.id}
            onClick={() => setCurrentPage(item.id)}
          />
        ))}
      </nav>

      {/* Bottom section */}
      <div className="px-3 pb-3">
        <div className="h-px bg-border" />
        <p className="mt-2 text-center text-xs text-muted-foreground">
          v2.0.0
        </p>
      </div>
    </div>
  );
}

function SidebarNavItem({
  item,
  active,
  onClick,
}: {
  item: NavItem;
  active: boolean;
  onClick: () => void;
}) {
  const Icon = item.icon;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={onClick}
          className={cn(
            "flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors",
            active
              ? "bg-primary/10 font-medium text-primary"
              : "text-muted-foreground hover:bg-muted hover:text-foreground",
          )}
        >
          <Icon className="h-4 w-4 flex-shrink-0" />
          <span className="truncate">{item.label}</span>
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">
        <p>{item.label}</p>
      </TooltipContent>
    </Tooltip>
  );
}
