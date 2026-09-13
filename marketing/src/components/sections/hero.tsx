import {
  AppleLogo,
  DownloadSimple,
  GithubLogo,
  Terminal,
  Desktop,
  WindowsLogo,
} from "@phosphor-icons/react/dist/ssr";
import { OrbitMark } from "@/components/brand";
import { Band, ButtonLink, SectionLabel, Shell } from "@/components/ui";

const platforms = [
  { Icon: AppleLogo, label: "macOS 13+" },
  { Icon: WindowsLogo, label: "Windows soon" },
  { Icon: Terminal, label: "Linux soon" },
];

const surfaces = [
  {
    Icon: Desktop,
    name: "Desktop",
    line: "A native GPUI workbench",
    link: "Download for macOS",
    href: "#install",
  },
  {
    Icon: Terminal,
    name: "Terminal",
    line: "The same pi sessions",
    link: "pi coding agent",
    href: "https://github.com/earendil-works/pi",
  },
  {
    Icon: GithubLogo,
    name: "Source",
    line: "All Rust, three commands",
    link: "View on GitHub",
    href: "https://github.com/rajeshwar-hyphun/orbit",
  },
];

export function Hero() {
  return (
    <Shell>
      <Band dashed className="pb-7 pt-14">
        <div className="flex flex-col gap-9">
          <SectionLabel>Native · Rust · GPU-drawn</SectionLabel>

          <div className="flex flex-col items-start gap-6">
            <h1 className="flex flex-col items-start text-[clamp(36px,4.2vw,52px)] font-[250] leading-[1.16] tracking-[-0.022em] text-ink">
              <span className="rise flex flex-wrap items-center gap-x-[0.23em] gap-y-[0.12em]">
                Get
                <OrbitMark className="block h-[0.82em] w-auto shrink-0 text-ink-3" />
                <span className="sr-only">Orbit</span>
                and run pi
              </span>
              <span className="rise" style={{ animationDelay: "90ms" }}>
                on <strong className="font-[450]">desktop, terminal, and GPU.</strong>
              </span>
            </h1>

            <p className="max-w-[52ch] text-[13px] leading-[1.85] text-ink-2">
              A native workbench for the pi coding agent.
              <br />
              Your sessions, your models, your machine.
            </p>

            <div className="flex flex-wrap items-center gap-3.5">
              <ButtonLink
                href="#install"
                variant="primary"
                className="min-w-[184px]"
              >
                <DownloadSimple className="size-[15px]" weight="bold" />
                Download for macOS
              </ButtonLink>
              <ButtonLink
                href="https://github.com/rajeshwar-hyphun/orbit"
                variant="secondary"
                external
              >
                <GithubLogo className="size-[15px]" />
                View on GitHub
              </ButtonLink>
            </div>

            <div className="flex flex-wrap items-center gap-x-[22px] gap-y-[18px]">
              {platforms.map(({ Icon, label }) => (
                <span
                  key={label}
                  className="inline-flex items-center gap-2 text-[11px] text-ink-2"
                >
                  <Icon className="h-4 w-3.5" />
                  {label}
                </span>
              ))}
            </div>
          </div>

          {/* three surfaces */}
          <div className="grid w-full max-w-[940px] grid-cols-1 gap-y-[18px] sm:grid-cols-3 sm:gap-y-0">
            {surfaces.map(({ Icon, name, line, link, href }, i) => (
              <div
                key={name}
                className={`relative flex flex-col items-center gap-1.5 px-7 text-center ${
                  i > 0
                    ? "border-t border-dashed border-[#ffffff1c] pt-[18px] sm:border-t-0 sm:pt-0 sm:pl-7"
                    : ""
                } ${
                  i > 0
                    ? "sm:shadow-[inset_1px_0_#ffffff1c]"
                    : ""
                }`}
              >
                <Icon className="mb-2 size-6 text-ink-3" />
                <span className="font-mono text-[11px] uppercase tracking-[0.16em] text-ink-3">
                  {name}
                </span>
                <span className="inline-flex min-h-9 items-center text-[13.5px] text-ink-2">
                  {line}
                </span>
                <a
                  href={href}
                  {...(href.startsWith("http")
                    ? { target: "_blank", rel: "noreferrer" }
                    : {})}
                  className="text-[13px] text-ink underline-offset-[3px] transition-colors hover:underline"
                >
                  {link}
                </a>
              </div>
            ))}
          </div>
        </div>
      </Band>
    </Shell>
  );
}
