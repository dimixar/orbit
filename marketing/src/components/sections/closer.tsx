import { DownloadSimple, GithubLogo } from "@phosphor-icons/react/dist/ssr";
import { Band, ButtonLink, Shell } from "@/components/ui";

export function Closer() {
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
          <ButtonLink href="#install" variant="primary" className="min-w-[184px]">
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
        </div>
      </Band>
    </Shell>
  );
}
