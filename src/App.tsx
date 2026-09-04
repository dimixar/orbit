import { useState } from "react";
import { SidebarInset } from "@/components/ui/sidebar";
import { SidebarProvider } from "@/components/ui/sidebar";
import AppSidebar from "@/components/app-sidebar";
import { PluginsPage } from "@/components/workbench/plugins-page";
import { ModelsPage } from "@/components/workbench/models-page";
import { SettingsPage } from "@/components/workbench/settings-page";
import { SkillsPage } from "@/components/workbench/skills-page";
import { UsagePage } from "@/components/workbench/usage-page";
import ChatPanel from "@/components/chat-panel";
import { AssistantRuntimeProvider } from "@assistant-ui/react";
import { usePiRuntime } from "@assistant-ui/react-pi";
import { piClient } from "@/lib/pi-client";
import { useTheme } from "@/hooks/use-theme";

export type WorkbenchView =
  | "chat"
  | "usage"
  | "skills"
  | "plugins"
  | "models"
  | "settings";

function App() {
  const { theme, setTheme, accent, setAccent } = useTheme();
  const [view, setView] = useState<WorkbenchView>(() => {
    // Allow deep-linking to a view via the URL hash, e.g. #usage.
    const hash = window.location.hash.replace("#", "") as WorkbenchView;
    return ["chat", "usage", "skills", "plugins", "models", "settings"].includes(hash)
      ? hash
      : "chat";
  });

  // The Pi runtime over SSE — the single source of truth for the chat. The
  // WebSocket daemon ("state stream") is no longer used.
  const runtime = usePiRuntime({ client: piClient });

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <SidebarProvider className="h-svh">
        <AppSidebar
          view={view}
          onNavigate={setView}
          onNewChat={() => setView("chat")}
        />
        <SidebarInset className="min-h-0">
          {view === "chat" && <ChatPanel />}
          {view === "usage" && <UsagePage />}
          {view === "skills" && <SkillsPage />}
          {view === "plugins" && <PluginsPage />}
          {view === "models" && <ModelsPage />}
          {view === "settings" && (
            <SettingsPage
              theme={theme}
              onThemeChange={setTheme}
              accent={accent}
              onAccentChange={setAccent}
            />
          )}
        </SidebarInset>
      </SidebarProvider>
    </AssistantRuntimeProvider>
  );
}

export default App;
