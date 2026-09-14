import { OrbitWordmark } from "@/components/brand";
import { Band, ButtonLink, Shell } from "@/components/ui";

const links = [
  { label: "Features", href: "#features" },
  { label: "Workbench", href: "#workbench" },
  { label: "Gallery", href: "#gallery" },
  { label: "Get started", href: "#install" },
];

export function SiteNav() {
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
              <ButtonLink href="#install" variant="nav">
                Download
              </ButtonLink>
            </div>

            <div className="md:hidden">
              <ButtonLink href="#install" variant="nav">
                Download
              </ButtonLink>
            </div>
          </nav>
        </Band>
      </Shell>
    </header>
  );
}
