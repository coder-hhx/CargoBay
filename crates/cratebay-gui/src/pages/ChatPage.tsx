import { MessageList } from "@/components/chat/MessageList";
import { ChatInput } from "@/components/chat/ChatInput";

export function ChatPage() {
  return (
    <div className="flex h-full flex-col">
      <MessageList />
      <ChatInput />
    </div>
  );
}
