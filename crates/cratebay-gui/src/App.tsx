import { useAppStore } from "@/stores/appStore";
import { AppLayout } from "@/components/layout/AppLayout";
import { ChatPage } from "@/pages/ChatPage";
import { ContainersPage } from "@/pages/ContainersPage";
import { McpPage } from "@/pages/McpPage";
import { SettingsPage } from "@/pages/SettingsPage";

function App() {
  const currentPage = useAppStore((s) => s.currentPage);

  return (
    <AppLayout>
      {currentPage === "chat" && <ChatPage />}
      {currentPage === "containers" && <ContainersPage />}
      {currentPage === "mcp" && <McpPage />}
      {currentPage === "settings" && <SettingsPage />}
    </AppLayout>
  );
}

export default App;
