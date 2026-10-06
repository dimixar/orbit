import { Shot } from "@/components/shot";
import { Story } from "@/components/sections/story";

export function Mcp() {
  return (
    <Story
      id="mcp"
      label="MCP"
      title="Wire in your tools over MCP."
      copy="Orbit manages Model Context Protocol servers for the agent — global or scoped to a project. Add a server, see its tool count and connection status, and flip a switch to enable or disable it."
      points={[
        "Global and project-scoped servers, side by side.",
        "Connection status and tool counts at a glance.",
        "Enable, edit, or remove a server without editing JSON.",
      ]}
      flip
      visual={
        <Shot
          src="/screens/mcp.png"
          srcLight="/screens/mcp-light.png"
          alt="Orbit's MCP page listing global and project MCP servers with connection status and enable toggles."
          sizes="(max-width: 1024px) 100vw, 560px"
        />
      }
    />
  );
}
