import { ArrowDown } from "@phosphor-icons/react/dist/ssr";
import { Band, SectionLabel, Shell } from "@/components/ui";

function Layer({
  label,
  title,
  chips,
  note,
}: {
  label: string;
  title: string;
  chips: string[];
  note: string;
}) {
  return (
    <div className="w-full rounded-[14px] bg-card p-4 text-left shadow-[inset_0_0_0_1px_#ffffff0a]">
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-ink-3">
          {label}
        </span>
        <span className="text-[13.5px] font-[450] text-ink">{title}</span>
      </div>
      <div className="mt-2.5 flex flex-wrap gap-1.5">
        {chips.map((c) => (
          <span
            key={c}
            className="rounded-md bg-[#ffffff0a] px-2 py-1 font-mono text-[10.5px] text-ink-2 shadow-[inset_0_0_0_1px_#ffffff0f]"
          >
            {c}
          </span>
        ))}
      </div>
      <p className="mt-2.5 text-[12px] text-ink-3">{note}</p>
    </div>
  );
}

function Connector({ caption }: { caption: string }) {
  return (
    <div className="flex flex-col items-center py-3">
      <span className="h-5 w-px bg-[#ffffff1c]" />
      <span className="flex items-center gap-2 text-ink-3">
        <span className="font-mono text-[10.5px]">{caption}</span>
        <ArrowDown className="size-3" />
      </span>
      <span className="h-5 w-px bg-[#ffffff1c]" />
    </div>
  );
}

export function Architecture() {
  return (
    <Shell>
      <Band
        id="architecture"
        dashed
        className="relative scroll-mt-20 overflow-hidden bg-missions py-16 sm:py-[88px]"
      >
        <div className="relative z-[1] grid grid-cols-1 items-center gap-12 lg:grid-cols-[1fr_1.05fr] lg:gap-16">
          <div>
            <SectionLabel>Architecture</SectionLabel>
            <h2 className="mb-3.5 mt-3 text-[clamp(26px,3vw,36px)] font-[300] leading-[1.2] tracking-[-0.02em] text-ink">
              Three layers, one process tree.
            </h2>
            <p className="mb-6 max-w-[48ch] text-[13.5px] leading-[1.8] text-ink-2">
              Orbit spawns the pi CLI as a child process and speaks its native
              newline-delimited JSON RPC over stdio — one process per open session.
              Everything above the RPC is Rust drawing to the GPU.
            </p>
            <div className="rounded-[14px] bg-well p-4 shadow-[inset_0_0_0_1px_#ffffff0f]">
              <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-ink-3">
                Removed, not a fallback
              </p>
              <p className="mt-2 max-w-[44ch] text-[13px] leading-[1.7] text-ink-2">
                The old React/Vite/Tauri web app, the Node daemon, the SSE surface,
                and the bundled sidecar are gone. The GPUI app is the only app.
              </p>
            </div>
          </div>

          <div className="flex flex-col items-center">
            <Layer
              label="Orbit"
              title="GPUI · Metal-rendered UI"
              chips={["window", "transcript", "markdown", "workbench"]}
              note="Pure Rust. No DOM, no CSS, no browser engine."
            />
            <Connector caption="JSON-RPC over stdio" />
            <Layer
              label="pi CLI"
              title="child process, per session"
              chips={["pi --mode rpc", "sessions", "models", "tools"]}
              note="The agent runtime is the pi binary you already run."
            />
            <Connector caption="reads / writes" />
            <Layer
              label="~/.pi/agent"
              title="pi's own store"
              chips={["sessions", "config", "usage"]}
              note="The same data the CLI reads. Orbit never forks it."
            />
          </div>
        </div>
      </Band>
    </Shell>
  );
}
