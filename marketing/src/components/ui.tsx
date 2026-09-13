import type { ReactNode } from "react";

export function Shell({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return <div className={`shell ${className}`}>{children}</div>;
}

export function Band({
  children,
  className = "",
  dashed = false,
  id,
}: {
  children: ReactNode;
  className?: string;
  dashed?: boolean;
  id?: string;
}) {
  return (
    <section
      id={id}
      className={`band ${dashed ? "band-dashed" : ""} ${className}`}
    >
      {children}
    </section>
  );
}

const base =
  "inline-flex items-center justify-center gap-[9px] whitespace-nowrap rounded-[5px] border border-transparent text-[12px] font-[550] no-underline transition-[background-color,border-color,box-shadow] duration-100 ease-out";

const variants = {
  primary:
    "min-h-10 bg-ink px-[18px] text-page border-white/60 hover:bg-white hover:border-white active:bg-[#e5e5e7]",
  secondary:
    "min-h-10 border-hair text-ink hover:bg-slab hover:border-white/20 active:bg-[#191a1e]",
  nav: "rounded-full bg-ink px-[15px] py-[7px] text-[13.5px] font-medium text-page hover:bg-white",
  text: "border-transparent px-0 text-[13px] text-ink-2 hover:text-ink",
} as const;

export function ButtonLink({
  href,
  children,
  variant = "primary",
  className = "",
  external = false,
}: {
  href: string;
  children: ReactNode;
  variant?: keyof typeof variants;
  className?: string;
  external?: boolean;
}) {
  return (
    <a
      href={href}
      className={`${base} ${variants[variant]} ${className}`}
      {...(external ? { target: "_blank", rel: "noreferrer" } : {})}
    >
      {children}
    </a>
  );
}

export function SectionLabel({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <p
      className={`flex items-center gap-[9px] font-mono text-[10px] uppercase leading-[1.4] tracking-[0.065em] text-ink-2 ${className}`}
    >
      <i className="pulse-dot size-[5px] shrink-0 rounded-full bg-accent" />
      {children}
    </p>
  );
}

export function Eyebrow({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <p
      className={`font-mono text-[11.5px] uppercase leading-normal tracking-[0.16em] text-ink-3 ${className}`}
    >
      {children}
    </p>
  );
}
