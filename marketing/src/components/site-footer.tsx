import { OrbitWordmark } from "@/components/brand";
import { Band, Shell } from "@/components/ui";

const links = [
  { label: "Features", href: "#features", external: false },
  { label: "Workbench", href: "#workbench", external: false },
  { label: "Safeguards", href: "#guard", external: false },
  { label: "Gallery", href: "#gallery", external: false },
  { label: "Get started", href: "#install", external: false },
  {
    label: "GitHub",
    href: "https://github.com/imrj05/orbit",
    external: true,
  },
  {
    label: "pi coding agent",
    href: "https://github.com/earendil-works/pi",
    external: true,
  },
];

export function SiteFooter() {
  return (
    <footer>
      <Shell>
        <Band className="flex flex-wrap items-center justify-between gap-5 py-7 pb-10">
          <div className="inline-flex items-center">
            <OrbitWordmark className="h-[26px] w-auto" alt="" />
          </div>

          <nav className="flex min-w-0 flex-wrap gap-x-[22px] gap-y-3 text-[13px]">
            {links.map((l) => (
              <a
                key={l.label}
                href={l.href}
                {...(l.external ? { target: "_blank", rel: "noreferrer" } : {})}
                className="text-ink-2 no-underline transition-colors duration-[120ms] hover:text-ink"
              >
                {l.label}
              </a>
            ))}
          </nav>

          <p className="font-mono text-[11.5px] text-ink-3">
            © {new Date().getFullYear()} Orbit · same sessions as the terminal
          </p>
        </Band>
      </Shell>
    </footer>
  );
}
