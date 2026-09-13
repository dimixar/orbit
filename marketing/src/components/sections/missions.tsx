import { Panel, SessionsMock } from "@/components/sections/mocks";
import { Band, SectionLabel, Shell } from "@/components/ui";

const phases = [
  {
    n: "01",
    title: "Sessions grouped by project",
    body: "Every run is filed under the working directory it started in. Reopen it, hide it, or clone it.",
  },
  {
    n: "02",
    title: "Streaming you can steer",
    body: "Prompt, queue a follow-up, steer mid-run, or cancel — all against the live pi process.",
  },
  {
    n: "03",
    title: "Tool receipts",
    body: "Bash, edit, and thinking rows render natively and expand into arguments and output.",
  },
  {
    n: "04",
    title: "Review before you switch apps",
    body: "A live git diff of the workspace sits next to the transcript, refreshed when a run settles.",
  },
];

export function Missions() {
  return (
    <Shell>
      <Band
        id="workbench"
        dashed
        className="relative scroll-mt-20 overflow-hidden bg-missions py-16 sm:py-[88px]"
      >
        <div className="relative z-[1] grid grid-cols-1 items-center gap-12 lg:grid-cols-2 lg:gap-16">
          <div>
            <SectionLabel>Workbench</SectionLabel>
            <h2 className="mb-3.5 mt-3 text-[clamp(26px,3vw,36px)] font-[300] leading-[1.2] tracking-[-0.02em] text-ink">
              One workbench. Every session.
            </h2>
            <p className="mb-8 max-w-[46ch] text-[13.5px] leading-[1.8] text-ink-2">
              Orbit keeps pi&apos;s own session truth and layers the workbench on
              top. The long-running session is a waypoint, not the destination.
            </p>

            <ol className="flex flex-col">
              {phases.map((p) => (
                <li
                  key={p.n}
                  className="flex gap-5 border-t border-[#ffffff0f] py-4"
                >
                  <span className="pt-0.5 font-mono text-[11px] text-ink-3">
                    {p.n}
                  </span>
                  <div>
                    <p className="text-[14px] font-[450] text-ink">{p.title}</p>
                    <p className="mt-1 max-w-[44ch] text-[13px] leading-[1.7] text-ink-2">
                      {p.body}
                    </p>
                  </div>
                </li>
              ))}
            </ol>
          </div>

          <Panel title="sessions · live" meta="6 parked" className="rise">
            <SessionsMock />
          </Panel>
        </div>
      </Band>
    </Shell>
  );
}
