import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useChatStore } from "@/stores/chatStore";
import { useI18n } from "@/lib/i18n";
import {
  MessageSquare,
  Box,
  Layers,
  Plug,
  Settings,
  Plus,
  Trash2,
  type LucideIcon,
} from "lucide-react";

type PageId = "chat" | "containers" | "images" | "mcp" | "settings";

interface NavItem {
  id: PageId;
  labelKey: "chat" | "containers" | "images" | "mcp" | "settings";
  icon: LucideIcon;
}

const navItems: NavItem[] = [
  { id: "chat", labelKey: "chat", icon: MessageSquare },
  { id: "containers", labelKey: "containers", icon: Box },
  { id: "images", labelKey: "images", icon: Layers },
  { id: "mcp", labelKey: "mcp", icon: Plug },
  { id: "settings", labelKey: "settings", icon: Settings },
];

export function Sidebar() {
  const { t } = useI18n();
  const currentPage = useAppStore((s) => s.currentPage);
  const setCurrentPage = useAppStore((s) => s.setCurrentPage);
  const sessions = useChatStore((s) => s.sessions);
  const activeSessionId = useChatStore((s) => s.activeSessionId);
  const setActiveSession = useChatStore((s) => s.setActiveSession);
  const createSession = useChatStore((s) => s.createSession);
  const deleteSession = useChatStore((s) => s.deleteSession);

  return (
    <div className="flex h-full w-full flex-col bg-card">
      {/* Logo header — aligned with TopBar breadcrumb row */}
      <div className="flex items-center gap-2.5 px-3 pb-[10px] pt-[34px]" data-tauri-drag-region>
        <img
          src="/logo.png"
          alt="CrateBay"
          className="h-7 w-7 flex-shrink-0"
          draggable={false}
        />
        <span
          data-testid="app-title"
          className="bg-gradient-to-r from-blue-500 to-purple-500 bg-clip-text text-sm font-semibold text-transparent"
        >
          CrateBay
        </span>
      </div>

      {/* Navigation items */}
      <nav className="flex flex-col gap-1 px-3">
        {navItems.map((item) => {
          const Icon = item.icon;
          const label = t("nav", item.labelKey);
          const active = currentPage === item.id;
          return (
            <button
              key={item.id}
              onClick={() => setCurrentPage(item.id)}
              data-testid={`nav-${item.id}`}
              aria-current={active ? "page" : undefined}
              className={cn(
                "flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors focus:outline-none",
                active
                  ? "bg-primary/10 font-medium text-primary"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground",
              )}
            >
              <Icon className="h-4 w-4 flex-shrink-0" />
              <span className="truncate">{label}</span>
            </button>
          );
        })}
      </nav>

      {/* Conversation history (only when on Chat page) */}
      {currentPage === "chat" && (
        <div className="mt-3 flex min-h-0 flex-1 flex-col px-3">
          <div className="mx-0 mb-2">
            <div className="h-px bg-border" />
          </div>
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              {t("sidebar", "conversations")}
            </span>
            <button
              type="button"
              onClick={() => void createSession()}
              data-testid="new-session"
              className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus:outline-none"
              title={t("sidebar", "newConversation")}
            >
              <Plus className="h-3.5 w-3.5" />
            </button>
          </div>

          <div
            className="flex flex-1 flex-col gap-0.5 overflow-y-auto"
            data-testid="session-list"
          >
            {sessions.length === 0 ? (
              <p className="py-4 text-center text-xs text-muted-foreground">
                {t("sidebar", "noConversations")}
              </p>
            ) : (
              sessions.map((session) => (
                <div
                  key={session.id}
                  data-testid="session-item"
                  className={cn(
                    "group relative flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left transition-colors",
                    session.id === activeSessionId
                      ? "bg-primary/10 text-primary"
                      : "text-muted-foreground hover:bg-muted hover:text-foreground",
                  )}
                >
                  <button
                    type="button"
                    onClick={() => void setActiveSession(session.id)}
                    className="flex min-w-0 flex-1 items-center gap-2 focus:outline-none"
                  >
                    <MessageSquare className="h-3.5 w-3.5 flex-shrink-0" />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-xs font-medium">
                        {session.title}
                      </div>
                      <div className="truncate text-[10px] opacity-60">
                        {formatSessionTime(session.updatedAt)}
                      </div>
                    </div>
                  </button>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      void deleteSession(session.id);
                    }}
                    data-testid="session-delete"
                    className="flex h-5 w-5 flex-shrink-0 items-center justify-center rounded opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive focus:outline-none group-hover:opacity-100"
                    title={t("sidebar", "deleteConversation")}
                  >
                    <Trash2 className="h-3 w-3" />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {/* Spacer */}
      {currentPage !== "chat" && <div className="flex-1" />}
    </div>
  );
}

function formatSessionTime(isoString: string): string {
  try {
    const date = new Date(isoString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    const diffHour = Math.floor(diffMs / 3600000);
    const diffDay = Math.floor(diffMs / 86400000);

    if (diffMin < 1) return "Just now";
    if (diffMin < 60) return `${diffMin}m ago`;
    if (diffHour < 24) return `${diffHour}h ago`;
    if (diffDay < 7) return `${diffDay}d ago`;
    return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  } catch {
    return "";
  }
}
