import { OrbitWordmark } from "@/components/brand";
import { Band, ButtonLink, Shell } from "@/components/ui";
import { getLatestRelease, LATEST_RELEASE_URL } from "@/lib/releases";

const links = [
  { label: "Features", href: "#features" },
  { label: "Workbench", href: "#workbench" },
  { label: "Safeguards", href: "#guard" },
  { label: "Gallery", href: "#gallery" },
  { label: "Get started", href: "#install" },
];

export async function SiteNav() {
  const release = await getLatestRelease();
  const downloadHref = release?.macos ?? LATEST_RELEASE_URL;

  return (
    <header className="sticky top-0 z-50 bg-page/80 backdrop-blur-md">
      <Shell>
        <Band dashed>
          <nav className="flex h-16 items-center justify-between">
            <a
              href="#top"
              aria-label="Orbit home"
              className="inline-flex items-center no-underline"
            >
              <OrbitWordmark className="h-[30px] w-auto" alt="" priority />
            </a>

            <div className="hidden items-center gap-[26px] text-[13.5px] md:flex">
              {links.map((l) => (
                <a
                  key={l.href}
                  href={l.href}
                  className="text-ink-2 no-underline transition-colors duration-[120ms] hover:text-ink"
                >
                  {l.label}
                </a>
              ))}
              <a
                href="https://github.com/imrj05/orbit"
                target="_blank"
                rel="noreferrer"
                className="text-ink no-underline transition-colors duration-[120ms] hover:text-ink-2"
              >
                GitHub
              </a>
              <ButtonLink href={downloadHref} variant="nav" external>
                Download
              </ButtonLink>
            </div>

            <div className="md:hidden">
              <ButtonLink href={downloadHref} variant="nav" external>
                Download
              </ButtonLink>
            </div>
          </nav>
        </Band>
      </Shell>
    </header>
  );
}
