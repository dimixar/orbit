import { ArrowRight } from "@phosphor-icons/react/dist/ssr";
import { CopyButton } from "@/components/copy-button";
import { Band, SectionLabel, Shell } from "@/components/ui";

const runCommand = "cargo run -p orbit-pi";
const buildCommand = `rustup target add aarch64-apple-darwin x86_64-apple-darwin
./scripts/make-dmg.sh universal`;

export function Quickstart() {
  return (
    <Shell>
      <Band
        id="install"
        dashed
        className="grid scroll-mt-20 grid-cols-1 items-start gap-7 py-11 sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] sm:gap-12 sm:py-[52px]"
      >
        <div>
          <SectionLabel>Build</SectionLabel>
          <h2 className="mb-3.5 mt-3 text-[clamp(24px,2.4vw,30px)] font-[350] leading-[1.25] tracking-[-0.025em] text-ink">
            From zero to a signed DMG.
          </h2>
          <p className="mb-[18px] text-[13px] leading-[1.8] text-ink-2">
            Orbit ships as source today. Run the app against your existing pi
            install, then package a signed, notarizable bundle for another Mac.
          </p>
          <a
            href="#install"
            className="inline-flex items-center gap-2 text-[11px] text-ink-2 no-underline transition-colors hover:text-ink"
          >
            Read the install guide
            <ArrowRight className="size-3.5" />
          </a>
        </div>

        <div className="min-w-0 overflow-hidden rounded-[14px] bg-window shadow-pop">
          <div className="flex items-center gap-2 bg-winbar px-3 py-[9px] text-[11px] text-ink-3 shadow-[inset_0_-1px_#00000066]">
            <span className="mr-2 inline-flex gap-1.5">
              <i className="size-[10px] rounded-full bg-[#ffffff29]" />
              <i className="size-[10px] rounded-full bg-[#ffffff29]" />
              <i className="size-[10px] rounded-full bg-[#ffffff29]" />
            </span>
            <span className="rounded-md bg-[#ffffff14] px-2.5 py-1 text-ink-2">
              shell
            </span>
          </div>
          <div className="flex items-center justify-between px-4 pt-3.5">
            <span className="font-mono text-[10.5px] uppercase tracking-[0.12em] text-ink-3">
              Install &amp; run
            </span>
            <CopyButton value={runCommand} />
          </div>
          <pre className="overflow-x-auto p-4 font-mono text-[12px] leading-[22px] text-ink-2">
            <code>
              <span className="text-ink-3">{"# 1 — run the app"}</span>
              {"\n"}
              <span className="text-accent">$</span> {runCommand}
              {"\n\n"}
              <span className="text-ink-3">{"# 2 — package a DMG"}</span>
              {"\n"}
              <span className="text-accent">$</span>{" "}
              {buildCommand.split("\n")[0]}
              {"\n"}
              <span className="text-accent">$</span>{" "}
              {buildCommand.split("\n")[1]}
            </code>
          </pre>
          <div className="flex items-center gap-4 border-t border-[#ffffff0f] px-4 py-2.5 font-mono text-[10.5px] text-ink-3">
            <span>Pure Rust</span>
            <span>no runtime to install</span>
            <span className="ml-auto">macOS 13+</span>
          </div>
        </div>
      </Band>
    </Shell>
  );
}
