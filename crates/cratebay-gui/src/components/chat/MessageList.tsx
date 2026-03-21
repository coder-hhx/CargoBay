import { useEffect, useRef, useCallback } from "react";
import { useChatStore } from "@/stores/chatStore";
import { useI18n } from "@/lib/i18n";
import { ScrollArea } from "@/components/ui/scroll-area";
import { MessageBubble } from "./MessageBubble";
import { Box, Database, Plug, Rocket } from "lucide-react";
import type { LucideIcon } from "lucide-react";

const EMPTY_MESSAGES: never[] = [];

interface Suggestion {
  icon: LucideIcon;
  titleKey: "suggestionCreateContainer" | "suggestionQueryDb" | "suggestionManageMcp" | "suggestionDeploy";
  descKey: "suggestionCreateContainerDesc" | "suggestionQueryDbDesc" | "suggestionManageMcpDesc" | "suggestionDeployDesc";
}

const suggestions: Suggestion[] = [
  { icon: Box, titleKey: "suggestionCreateContainer", descKey: "suggestionCreateContainerDesc" },
  { icon: Database, titleKey: "suggestionQueryDb", descKey: "suggestionQueryDbDesc" },
  { icon: Plug, titleKey: "suggestionManageMcp", descKey: "suggestionManageMcpDesc" },
  { icon: Rocket, titleKey: "suggestionDeploy", descKey: "suggestionDeployDesc" },
];

function CrateBayLogo({ size = 64 }: { size?: number }) {
  return (
    <img
      src="/logo.png"
      alt="CrateBay"
      width={size}
      height={size}
      className="rounded-xl"
    />
  );
}

interface WelcomeScreenProps {
  onSuggestionClick: (title: string) => void;
}

function WelcomeScreen({ onSuggestionClick }: WelcomeScreenProps) {
  const { t } = useI18n();

  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 p-10 text-center">
      <CrateBayLogo size={64} />
      <h2 className="bg-gradient-to-br from-[#7c3aed] to-[#22d3ee] bg-clip-text text-2xl font-bold text-transparent">
        {t("chat", "welcomeTitle")}
      </h2>
      <p className="max-w-[400px] text-sm text-muted-foreground">
        {t("chat", "welcomeDesc")}
      </p>
      <div className="mt-4 grid w-full max-w-[600px] grid-cols-2 gap-3">
        {suggestions.map((s) => {
          const Icon = s.icon;
          const title = t("chat", s.titleKey);
          const desc = t("chat", s.descKey);
          return (
            <button
              key={s.titleKey}
              type="button"
              onClick={() => onSuggestionClick(title)}
              className="group flex flex-col gap-1 rounded-xl border border-border bg-card p-4 text-left transition-all duration-150 hover:border-primary hover:bg-primary/5"
            >
              <div className="flex items-center gap-2">
                <Icon className="h-4 w-4 text-primary" />
                <span className="text-sm font-semibold text-foreground">
                  {title}
                </span>
              </div>
              <span className="text-xs text-muted-foreground">{desc}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function MessageList() {
  const activeSessionId = useChatStore((s) => s.activeSessionId);
  const messages = useChatStore((s) =>
    activeSessionId !== null
      ? (s.messages[activeSessionId] ?? EMPTY_MESSAGES)
      : EMPTY_MESSAGES,
  );
  const isStreaming = useChatStore((s) => s.isStreaming);
  const setInputDraft = useChatStore((s) => s.setInputDraft);
  const bottomRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const userScrolledUp = useRef(false);

  // Auto-scroll to bottom on new messages (unless user scrolled up)
  useEffect(() => {
    if (!userScrolledUp.current) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages, isStreaming]);

  // Detect user scrolling up
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const handleScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = el;
      userScrolledUp.current = scrollHeight - scrollTop - clientHeight > 100;
    };

    el.addEventListener("scroll", handleScroll);
    return () => el.removeEventListener("scroll", handleScroll);
  }, []);

  const handleSuggestionClick = useCallback(
    (title: string) => {
      setInputDraft(title);
    },
    [setInputDraft],
  );

  if (messages.length === 0) {
    return <WelcomeScreen onSuggestionClick={handleSuggestionClick} />;
  }

  return (
    <ScrollArea className="flex-1">
      <div
        ref={containerRef}
        className="mx-auto flex max-w-[800px] flex-col gap-5 px-4 py-6"
      >
        {messages.map((message) => (
          <MessageBubble key={message.id} message={message} />
        ))}
        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}
