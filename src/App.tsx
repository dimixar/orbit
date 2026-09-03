import { useEffect, useState } from "react";
import { SidebarInset } from "@/components/ui/sidebar";
import { SidebarProvider } from "@/components/ui/sidebar";
import AppSidebar from "@/components/app-sidebar";
import ChatPanel, { type ChatMessage } from "@/components/chat-panel";
import { piAgent, type PiAgentState, type PiSessionGroup } from "@/lib/pi-agent";

function App() {
  const [agentState, setAgentState] = useState<PiAgentState>({
    connected: false,
    isStreaming: false,
  });
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [sessionGroups, setSessionGroups] = useState<PiSessionGroup[]>([]);
  const [activeSessionPath, setActiveSessionPath] = useState<string | undefined>(
    undefined,
  );

  useEffect(() => {
    piAgent.connect();

    const unsubs = [
      piAgent.on("status", (connected) => {
        setAgentState((s) => ({ ...s, connected }));
        if (connected) piAgent.requestState();
      }),
      piAgent.on("ready", ({ sessionId }) => {
        setAgentState((s) => ({ ...s, sessionId }));
        piAgent.requestState();
        piAgent.requestSessions();
      }),
      piAgent.on("state", ({ model, isStreaming }) => {
        setAgentState((s) => ({ ...s, model, isStreaming }));
      }),
      piAgent.on("delta", ({ text }) => {
        setAgentState((s) => ({ ...s, isStreaming: true }));
        setMessages((msgs) => {
          const last = msgs[msgs.length - 1];
          if (last?.role === "assistant") {
            return [...msgs.slice(0, -1), { role: "assistant", text: last.text + text }];
          }
          return [...msgs, { role: "assistant", text }];
        });
      }),
      piAgent.on("agent_end", () => {
        setAgentState((s) => ({ ...s, isStreaming: false }));
      }),
      piAgent.on("sessions", ({ groups }) => {
        setSessionGroups(groups);
      }),
      piAgent.on("session_opened", ({ path }) => {
        setActiveSessionPath(path);
        setMessages([]);
        setAgentState((s) => ({ ...s, isStreaming: false }));
      }),
      piAgent.on("error", ({ message }) => {
        setAgentState((s) => ({ ...s, isStreaming: false }));
        setMessages((msgs) => [
          ...msgs,
          { role: "assistant", text: `⚠️ ${message}` },
        ]);
      }),
    ];

    return () => {
      for (const off of unsubs) off();
      piAgent.disconnect();
    };
  }, []);

  const send = (text: string) => {
    if (agentState.isStreaming) return;
    setMessages((msgs) => [...msgs, { role: "user", text }]);
    setAgentState((s) => ({ ...s, isStreaming: true }));
    piAgent.prompt(text);
  };

  const newChat = () => {
    piAgent.abort();
    setActiveSessionPath(undefined);
    setMessages([]);
  };

  const openSession = (path: string) => {
    piAgent.openSession(path);
  };

  return (
    <SidebarProvider>
      <AppSidebar
        agentState={agentState}
        sessionGroups={sessionGroups}
        activeSessionPath={activeSessionPath}
        onNewChat={newChat}
        onOpenSession={openSession}
      />
      <SidebarInset>
        <ChatPanel
          agentState={agentState}
          messages={messages}
          sessionGroups={sessionGroups}
          activeSessionPath={activeSessionPath}
          onSend={send}
          onAbort={() => piAgent.abort()}
        />
      </SidebarInset>
    </SidebarProvider>
  );
}

export default App;
