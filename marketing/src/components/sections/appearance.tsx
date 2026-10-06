import { Shot } from "@/components/shot";
import { Story } from "@/components/sections/story";

export function Appearance() {
  return (
    <Story
      id="appearance"
      label="Appearance"
      title="Make it feel like yours."
      copy="Light and dark themes, interface and code fonts, sizes, and language — Appearance covers the window itself, and every other setting is one search away."
      points={[
        "Light and dark themes with per-theme palettes.",
        "Interface and code fonts, sizes, and a live terminal preview.",
        "Search across every setting from one field.",
      ]}
      visual={
        <Shot
          src="/screens/appearance.png"
          srcLight="/screens/appearance-light.png"
          alt="Orbit's Appearance settings with theme choices above interface font, code font, and size controls."
          sizes="(max-width: 1024px) 100vw, 560px"
        />
      }
    />
  );
}
