import { DownloadSimple, GithubLogo } from "@phosphor-icons/react/dist/ssr";
import { Band, ButtonLink, Shell } from "@/components/ui";
import { getLatestRelease, LATEST_RELEASE_URL } from "@/lib/releases";

export async function Closer() {
  const release = await getLatestRelease();
  const downloadHref = release?.macos ?? LATEST_RELEASE_URL;
  const platforms = [
    { label: "macOS", href: release?.macos ?? LATEST_RELEASE_URL },
    { label: "Windows", href: release?.windows ?? LATEST_RELEASE_URL },
    { label: "Linux", href: release?.linux ?? LATEST_RELEASE_URL },
  ];

  return (
    <Shell>
      <Band dashed className="flex flex-col items-center py-16 text-center sm:py-[88px]">
        <h2 className="mb-4 max-w-[22ch] text-[clamp(30px,3.8vw,44px)] font-[250] leading-[1.14] tracking-[-0.015em] text-ink">
          Start it tonight. Keep your sessions.
        </h2>
        <p className="mb-8 max-w-[52ch] text-[15px] leading-[1.7] text-ink-2">
          Point Orbit at the projects you already run pi on. Same sessions, same
          models, same tools — just drawn on the GPU.
        </p>
        <div className="flex flex-wrap items-center justify-center gap-3.5">
          <ButtonLink
            href={downloadHref}
            variant="primary"
            className="min-w-[184px]"
            external
          >
            <DownloadSimple data-icon="inline-start" weight="bold" />
            {release ? `Download ${release.tag}` : "Download for macOS"}
          </ButtonLink>
          <ButtonLink
            href="https://github.com/imrj05/orbit"
            variant="secondary"
            external
          >
            <GithubLogo data-icon="inline-start" />
            View on GitHub
          </ButtonLink>
        </div>
        <div className="mt-4 flex flex-wrap items-center justify-center gap-x-4 gap-y-2 font-mono text-[11px] uppercase tracking-[0.12em] text-ink-3">
          {platforms.map(({ label, href }) => (
            <a
              key={label}
              href={href}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1.5 no-underline transition-colors hover:text-ink"
            >
              <DownloadSimple className="size-3.5" weight="bold" />
              {label}
            </a>
          ))}
          {release ? (
            <a
              href={release.pageUrl}
              target="_blank"
              rel="noreferrer"
              className="no-underline transition-colors hover:text-ink"
            >
              Release notes · {release.tag}
            </a>
          ) : null}
        </div>
      </Band>
    </Shell>
  );
}
