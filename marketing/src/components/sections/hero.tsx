import {
  AppleLogo,
  DownloadSimple,
  GithubLogo,
  Terminal,
  Desktop,
  GitDiff,
  Plugs,
  WindowsLogo,
} from "@phosphor-icons/react/dist/ssr";
import { OrbitIcon } from "@/components/brand";
import { Band, ButtonLink, SectionLabel, Shell } from "@/components/ui";
import { getLatestRelease, LATEST_RELEASE_URL } from "@/lib/releases";

export async function Hero() {
  const release = await getLatestRelease();
  const downloadHref = release?.macos ?? LATEST_RELEASE_URL;

  const platforms = [
    { Icon: AppleLogo, label: "macOS 13+", meta: "universal .dmg", href: release?.macos },
    { Icon: WindowsLogo, label: "Windows", meta: "x86_64 .zip", href: release?.windows },
    { Icon: Terminal, label: "Linux", meta: "x86_64 .tar.gz", href: release?.linux },
  ];

  const surfaces = [
    {
      Icon: Desktop,
      name: "Desktop",
      line: "A native workbench for pi",
      link: release ? `Download ${release.tag}` : "Download for macOS",
      href: downloadHref,
    },
    {
      Icon: Plugs,
      name: "Providers",
      line: "Ollama, Anthropic, Bedrock",
      link: "See providers",
      href: "#workbench",
    },
    {
      Icon: GitDiff,
      name: "Git",
      line: "Review, commit, and push",
      link: "See the flow",
      href: "#gallery",
    },
  ];

  return (
    <Shell>
      <Band dashed className="pb-7 pt-14">
        <div className="flex flex-col gap-9">
          <SectionLabel>Sessions · Providers · Git</SectionLabel>

          <div className="flex flex-col items-start gap-6">
            <h1 className="flex flex-col items-start text-[clamp(36px,4.2vw,52px)] font-[250] leading-[1.16] tracking-[-0.022em] text-ink">
              <span className="rise flex flex-wrap items-center gap-x-[0.23em] gap-y-[0.12em]">
                Get
                <OrbitIcon className="block h-[0.95em] w-auto shrink-0" alt="" />
                <span className="sr-only">Orbit</span>
                and run pi
              </span>
              <span className="rise" style={{ animationDelay: "90ms" }}>
                on <strong className="font-[450]">desktop, terminal, and every workspace.</strong>
              </span>
            </h1>

            <p className="max-w-[52ch] text-[13px] leading-[1.85] text-ink-2">
              A native workbench for the pi coding agent.
              <br />
              Your sessions, your models, your machine.
            </p>

            <div className="flex flex-wrap items-center gap-3.5">
              <ButtonLink
                href={downloadHref}
                variant="primary"
                className="min-w-[184px]"
                external
              >
                <DownloadSimple data-icon="inline-start" weight="bold" />
                Download for macOS
              </ButtonLink>
              <ButtonLink
                href="https://github.com/imrj05/orbit"
                variant="secondary"
                external
              >
                <GithubLogo data-icon="inline-start" />
                View on GitHub
              </ButtonLink>
              {release ? (
                <a
                  href={release.pageUrl}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex items-center gap-2 font-mono text-[11px] uppercase tracking-[0.12em] text-ink-3 no-underline transition-colors hover:text-ink"
                >
                  <span className="rounded-full border border-[#ffffff1c] px-2.5 py-1">
                    {release.tag}
                  </span>
                  <span>Release notes</span>
                </a>
              ) : null}
            </div>

            <div className="flex flex-wrap items-center gap-x-5 gap-y-[14px]">
              {platforms.map(({ Icon, label, meta, href }) => {
                const body = (
                  <>
                    <Icon className="h-4 w-3.5" />
                    <span>{label}</span>
                    {meta ? (
                      <span className="font-mono text-[10px] uppercase tracking-[0.08em] text-ink-3">
                        {meta}
                      </span>
                    ) : null}
                  </>
                );
                return href ? (
                  <a
                    key={label}
                    href={href}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-2 text-[11px] text-ink-2 no-underline underline-offset-[3px] transition-colors hover:text-ink hover:underline"
                  >
                    {body}
                  </a>
                ) : (
                  <span
                    key={label}
                    className="inline-flex items-center gap-2 text-[11px] text-ink-2"
                  >
                    {body}
                  </span>
                );
              })}
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
