import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { StatusBar } from "./StatusBar";
import { TooltipProvider } from "@/components/ui/tooltip";

interface AppLayoutProps {
  children: React.ReactNode;
}

export function AppLayout({ children }: AppLayoutProps) {
  const sidebarOpen = useAppStore((s) => s.sidebarOpen);
  const sidebarWidth = useAppStore((s) => s.sidebarWidth);

  return (
    <TooltipProvider>
      <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground">
        {/* Sidebar */}
        <aside
          className={cn(
            "flex-shrink-0 border-r border-border transition-[width] duration-200 ease-in-out",
            sidebarOpen ? "overflow-hidden" : "w-0 border-r-0",
          )}
          style={sidebarOpen ? { width: `${sidebarWidth}px` } : undefined}
        >
          <Sidebar />
        </aside>

        {/* Main content area */}
        <div className="flex min-w-0 flex-1 flex-col">
          <TopBar />

          <main className="flex-1 overflow-hidden">{children}</main>

          <StatusBar />
        </div>
      </div>
    </TooltipProvider>
  );
}
