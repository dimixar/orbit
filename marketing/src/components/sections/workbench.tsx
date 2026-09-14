import {
  BracketsCurly,
  ChartLine,
  Cube,
  Palette,
  Plugs,
  Sparkle,
} from "@phosphor-icons/react/dist/ssr";
import { Card } from "@/components/ui/card";
import { Band, SectionLabel, Shell } from "@/components/ui";

const features = [
  {
    Icon: Plugs,
    title: "Providers",
    body: "pi's live catalog plus custom providers — sign in with a key or OAuth, and watch usage by window.",
  },
  {
    Icon: Cube,
    title: "Plugins",
    body: "Install pi packages from npm, git, or a local path — global or per project — and update or remove them in place.",
  },
  {
    Icon: BracketsCurly,
    title: "Models",
    body: "Pick from pi's model catalog per task, with thinking effort set alongside it.",
  },
  {
    Icon: Sparkle,
    title: "Skills",
    body: "The skills pi has loaded for this machine and this project.",
  },
  {
    Icon: ChartLine,
    title: "Usage",
    body: "Tokens, sessions, and cost from pi's own meter — with cached and context figures.",
  },
  {
    Icon: Palette,
    title: "Appearance",
    body: "Theme palette, background image, fonts, sizes, spacing density, and language.",
  },
];

export function Workbench() {
  return (
    <Shell>
      <Band
        id="workbench"
        dashed
        className="relative scroll-mt-20 overflow-hidden bg-missions py-16 sm:py-[88px]"
      >
        <div className="relative z-[1]">
          <SectionLabel>Workbench</SectionLabel>
          <h2 className="mb-3.5 mt-3 max-w-[20ch] text-[clamp(26px,3vw,36px)] font-[300] leading-[1.2] tracking-[-0.02em] text-ink">
            Everything around the chat.
          </h2>
          <p className="mb-10 max-w-[52ch] text-[13.5px] leading-[1.8] text-ink-2">
            Chat is one page. Orbit also gives you the provider, model, plugin, and
            appearance settings that keep the agent running the way you want — all
            reading pi&apos;s own data.
          </p>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {features.map(({ Icon, title, body }) => (
              <Card
                key={title}
                className="gap-3 rounded-[18px] p-5 ring-0"
              >
                <Icon className="size-5 text-ink-3" />
                <h3 className="text-[15px] font-medium text-ink">{title}</h3>
                <p className="text-[13.5px] leading-[1.6] text-ink-2">{body}</p>
              </Card>
            ))}
          </div>
        </div>
      </Band>
    </Shell>
  );
}
