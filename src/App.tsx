import { useCallback, useEffect, useState } from "react";
import { SidebarInset } from "@/components/ui/sidebar";
import { SidebarProvider } from "@/components/ui/sidebar";
import AppSidebar from "@/components/app-sidebar";
import { TopBar } from "@/components/top-bar";
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

const VIEWS: WorkbenchView[] = [
  "chat",
  "usage",
  "skills",
  "plugins",
  "models",
  "settings",
];

/** Workbench views visited so far — powers the top bar's back/forward. */
interface NavState {
  view: WorkbenchView;
  back: WorkbenchView[];
  forward: WorkbenchView[];
}

function App() {
  const {
    theme,
    setTheme,
    accent,
    setAccent,
    uiFont,
    setUiFont,
    codeFont,
    setCodeFont,
    uiScale,
    setUiScale,
    codeSize,
    setCodeSize,
    density,
    setDensity,
  } = useTheme();
  const [nav, setNav] = useState<NavState>(() => {
    // Allow deep-linking to a view via the URL hash, e.g. #usage.
    const hash = window.location.hash.replace("#", "") as WorkbenchView;
    const view = VIEWS.includes(hash) ? hash : "chat";
    return { view, back: [], forward: [] };
  });
  const { view } = nav;

  const navigate = useCallback((next: WorkbenchView) => {
    setNav(({ view, back, forward }) =>
      next === view
        ? { view, back, forward }
        : { view: next, back: [...back, view], forward: [] },
    );
  }, []);

  const goBack = useCallback(() => {
    setNav(({ view, back, forward }) => {
      if (back.length === 0) return { view, back, forward };
      return {
        view: back[back.length - 1],
        back: back.slice(0, -1),
        forward: [view, ...forward],
      };
    });
  }, []);

  const goForward = useCallback(() => {
    setNav(({ view, back, forward }) => {
      if (forward.length === 0) return { view, back, forward };
      return {
        view: forward[0],
        back: [...back, view],
        forward: forward.slice(1),
      };
    });
  }, []);

  // ⌘[ / ⌘] (Ctrl on other platforms) walk the view history, like a browser.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey) return;
      if (e.key === "[") {
        e.preventDefault();
        goBack();
      } else if (e.key === "]") {
        e.preventDefault();
        goForward();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [goBack, goForward]);

  // The Pi runtime over SSE — the single source of truth for the chat. The
  // WebSocket daemon ("state stream") is no longer used.
  const runtime = usePiRuntime({ client: piClient });

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <SidebarProvider className="h-svh">
        <AppSidebar
          view={view}
          onNavigate={navigate}
          onNewChat={() => navigate("chat")}
        />
        <SidebarInset className="min-h-0">
          {view !== "chat" && (
            <TopBar
              canGoBack={nav.back.length > 0}
              canGoForward={nav.forward.length > 0}
              onBack={goBack}
              onForward={goForward}
            />
          )}
          {view === "chat" && (
            <ChatPanel
              canGoBack={nav.back.length > 0}
              canGoForward={nav.forward.length > 0}
              onBack={goBack}
              onForward={goForward}
            />
          )}
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
              uiFont={uiFont}
              onUiFontChange={setUiFont}
              codeFont={codeFont}
              onCodeFontChange={setCodeFont}
              uiScale={uiScale}
              onUiScaleChange={setUiScale}
              codeSize={codeSize}
              onCodeSizeChange={setCodeSize}
              density={density}
              onDensityChange={setDensity}
            />
          )}
        </SidebarInset>
      </SidebarProvider>
    </AssistantRuntimeProvider>
  );
}

export default App;
