/**
 * TopBar — Top navigation bar with breadcrumbs and page-specific actions.
 *
 * When on ChatPage, displays:
 * - Sidebar toggle
 * - Editable session title (click to edit, Enter/blur to save)
 * - "New Chat" button
 * - Settings shortcut
 *
 * When on other pages, displays:
 * - Sidebar toggle
 * - Static page breadcrumb
 *
 * @see frontend-spec.md §5.1 — ChatPage TopBar: "session title, new chat, settings"
 */

import { useState, useRef, useCallback, useEffect } from "react";
import { useAppStore } from "@/stores/appStore";
import { useChatStore } from "@/stores/chatStore";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import {
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Settings,
  Pencil,
  Check,
  X,
} from "lucide-react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

// Page labels are now resolved via i18n nav section

export function TopBar() {
  const { t } = useI18n();
  const currentPage = useAppStore((s) => s.currentPage);
  const sidebarOpen = useAppStore((s) => s.sidebarOpen);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);

  return (
    <header className="flex flex-shrink-0 flex-col">
      {/* Drag region for macOS overlay titlebar */}
      <div className="h-8 flex-shrink-0" data-tauri-drag-region />
      <div className="flex h-12 items-center gap-3 border-b border-border px-4">
      {/* Sidebar toggle */}
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={toggleSidebar}
        aria-label={sidebarOpen ? t("topbar", "collapseSidebar") : t("topbar", "expandSidebar")}
      >
        {sidebarOpen ? (
          <PanelLeftClose className="h-4 w-4" />
        ) : (
          <PanelLeftOpen className="h-4 w-4" />
        )}
      </Button>

      {currentPage === "chat" ? (
        <ChatTopBarContent />
      ) : (
        <DefaultTopBarContent currentPage={currentPage} />
      )}
      </div>
    </header>
  );
}

/**
 * Default breadcrumb content for non-chat pages.
 */
function DefaultTopBarContent({ currentPage }: { currentPage: string }) {
  const { t } = useI18n();
  return (
    <div className="flex items-center gap-1.5 text-sm">
      <span className="text-muted-foreground">CrateBay</span>
      <span className="text-muted-foreground">/</span>
      <span className="font-medium text-foreground">
        {t("nav", currentPage as "chat" | "containers" | "images" | "mcp" | "settings")}
      </span>
    </div>
  );
}

/**
 * Chat-specific TopBar content with session title editing and new chat button.
 */
function ChatTopBarContent() {
  const { t } = useI18n();
  const sessions = useChatStore((s) => s.sessions);
  const activeSessionId = useChatStore((s) => s.activeSessionId);
  const createSession = useChatStore((s) => s.createSession);
  const updateSessionTitle = useChatStore((s) => s.updateSessionTitle);
  const setCurrentPage = useAppStore((s) => s.setCurrentPage);

  const activeSession = sessions.find((s) => s.id === activeSessionId) ?? null;
  const sessionTitle = activeSession?.title ?? t("chat", "newSession");

  // Inline title editing state
  const [isEditing, setIsEditing] = useState(false);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus input when entering edit mode
  useEffect(() => {
    if (isEditing && inputRef.current !== null) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [isEditing]);

  const startEditing = useCallback(() => {
    setEditValue(sessionTitle);
    setIsEditing(true);
  }, [sessionTitle]);

  const saveTitle = useCallback(() => {
    const trimmed = editValue.trim();
    if (trimmed.length > 0 && activeSessionId !== null && trimmed !== sessionTitle) {
      updateSessionTitle(activeSessionId, trimmed);
    }
    setIsEditing(false);
  }, [editValue, activeSessionId, sessionTitle, updateSessionTitle]);

  const cancelEditing = useCallback(() => {
    setIsEditing(false);
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        e.preventDefault();
        saveTitle();
      }
      if (e.key === "Escape") {
        e.preventDefault();
        cancelEditing();
      }
    },
    [saveTitle, cancelEditing],
  );

  const handleNewChat = useCallback(() => {
    createSession();
  }, [createSession]);

  return (
    <>
      {/* Session title (editable) */}
      <div className="flex min-w-0 flex-1 items-center gap-1.5">
        {isEditing ? (
          <div className="flex items-center gap-1">
            <input
              ref={inputRef}
              type="text"
              value={editValue}
              onChange={(e) => setEditValue(e.target.value)}
              onKeyDown={handleKeyDown}
              onBlur={saveTitle}
              className="h-7 max-w-[300px] rounded-md border border-primary/40 bg-transparent px-2 text-sm font-medium text-foreground outline-none ring-1 ring-primary/20"
              maxLength={64}
            />
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={saveTitle}
              aria-label={t("topbar", "saveTitle")}
              className="h-6 w-6"
            >
              <Check className="h-3.5 w-3.5 text-success" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              onMouseDown={(e) => e.preventDefault()}
              onClick={cancelEditing}
              aria-label={t("topbar", "cancelEditing")}
              className="h-6 w-6"
            >
              <X className="h-3.5 w-3.5 text-muted-foreground" />
            </Button>
          </div>
        ) : (
          <button
            type="button"
            onClick={startEditing}
            className="group flex min-w-0 items-center gap-1.5 rounded-md px-1.5 py-1 transition-colors hover:bg-muted"
            title={t("topbar", "clickToRename")}
          >
            <span className="truncate text-sm font-medium text-foreground">
              {sessionTitle}
            </span>
            <Pencil className="h-3 w-3 flex-shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
          </button>
        )}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1">
        {/* New Chat button */}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={handleNewChat}
              aria-label={t("common", "newChat")}
            >
              <Plus className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            <p>{t("common", "newChat")}</p>
          </TooltipContent>
        </Tooltip>

        {/* Settings shortcut */}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => setCurrentPage("settings")}
              aria-label={t("topbar", "settings")}
            >
              <Settings className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            <p>{t("topbar", "settings")}</p>
          </TooltipContent>
        </Tooltip>
      </div>
    </>
  );
}
