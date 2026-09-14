import { Shot } from "@/components/shot";
import { Band, Eyebrow, Shell } from "@/components/ui";

const shots = [
  {
    src: "/screens/new-task.png",
    kicker: "New task",
    caption: "Pick a workspace, then describe the task.",
    alt: "Orbit's new-task page with a workspace picker and the composer.",
    wide: true,
  },
  {
    src: "/screens/git.png",
    kicker: "Git",
    caption: "Stage, review, and commit — with History and Graph.",
    alt: "Orbit's Git page listing staged and changed files with line counts and a commit box.",
    wide: false,
  },
  {
    src: "/screens/providers.png",
    kicker: "Providers",
    caption: "pi's live catalog plus custom providers, with login and usage.",
    alt: "Orbit's Providers page showing provider cards for Ollama, OpenCode Go, Bedrock, and Anthropic with usage meters.",
    wide: false,
  },
  {
    src: "/screens/plugins.png",
    kicker: "Plugins",
    caption: "pi packages for this machine and this project.",
    alt: "Orbit's Plugins page listing installed pi packages with update and remove actions.",
    wide: false,
  },
  {
    src: "/screens/appearance.png",
    kicker: "Appearance",
    caption: "Themes, background, fonts, sizes, and spacing density.",
    alt: "Orbit's Appearance settings with theme swatches, type and density previews, and font pickers.",
    wide: false,
  },
];

export function Gallery() {
  return (
    <Shell>
      <Band id="gallery" dashed className="scroll-mt-20 py-16 sm:py-[88px]">
        <Eyebrow className="mb-3.5">Surfaces</Eyebrow>
        <h2 className="mb-4 max-w-[22ch] text-[clamp(28px,3.2vw,38px)] font-[300] leading-[1.2] tracking-[-0.015em] text-ink">
          Every surface, one window.
        </h2>
        <p className="mb-10 max-w-[52ch] text-[14.5px] leading-[1.7] text-ink-2">
          Sessions, Git, providers, plugins, and settings live in the same app —
          no context switching, no web views.
        </p>

        <div className="grid grid-cols-1 gap-5 md:grid-cols-2">
          {shots.map((s) => (
            <figure key={s.kicker} className={s.wide ? "md:col-span-2" : ""}>
              <Shot
                src={s.src}
                height={1499}
                alt={s.alt}
                sizes={
                  s.wide
                    ? "(max-width: 768px) 100vw, 1180px"
                    : "(max-width: 768px) 100vw, 590px"
                }
                className="transition-shadow duration-200"
              />
              <figcaption className="mt-3 flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <span className="font-mono text-[11px] uppercase tracking-[0.12em] text-ink-3">
                  {s.kicker}
                </span>
                <span className="text-[13.5px] text-ink-2">{s.caption}</span>
              </figcaption>
            </figure>
          ))}
        </div>
      </Band>
    </Shell>
  );
}
