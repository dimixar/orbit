import { Shot } from "@/components/shot";
import { Story } from "@/components/sections/story";

export function Plugins() {
  return (
    <Story
      id="plugins"
      label="Plugins"
      title="Extend pi with packages."
      copy="Install pi packages from npm, git, or a local path — globally or for this project. Orbit shows what's configured and its version, updates it, and pi picks changes up at its next restart."
      points={[
        "npm, git, or local-path packages, global or project scope.",
        "Install, update, and remove without leaving the window.",
        "Loaded at startup — restart pi to apply changes.",
      ]}
      visual={
        <Shot
          src="/screens/plugins.png"
          srcLight="/screens/plugins-light.png"
          alt="Orbit's Plugins page listing installed pi packages with an install field above and update and remove actions per package."
          sizes="(max-width: 1024px) 100vw, 560px"
        />
      }
    />
  );
}
