import type { ReactNode } from "react";
import { Band, Eyebrow, Shell } from "@/components/ui";
import {
  SessionsMock,
  ToolsMock,
  TranscriptMock,
} from "@/components/sections/mocks";

const cards: {
  kicker: string;
  title: string;
  body: string;
  art: ReactNode;
}[] = [
  {
    kicker: "Transcript",
    title: "Streaming that never reflows.",
    body: "A GPU-rendered list whose cost is independent of message count, with coalesced commits and paint-only syntax highlighting.",
    art: <TranscriptMock />,
  },
  {
    kicker: "Sessions",
    title: "Sessions live where pi put them.",
    body: "Orbit is a client, not a second source of truth. Your list is read from pi and grouped by the directory it ran in.",
    art: <SessionsMock />,
  },
  {
    kicker: "Tools",
    title: "Every tool call, rendered.",
    body: "Bash, edit, read, and thinking rows drawn natively, expanding into argument and output cards with per-section copy.",
    art: <ToolsMock />,
  },
];

export function Features() {
  return (
    <Shell>
      <Band id="features" dashed className="scroll-mt-20 py-16 text-center sm:py-20">
        <Eyebrow className="mb-3.5">Built for the long run</Eyebrow>
        <h2 className="mx-auto mb-10 max-w-[20ch] text-[clamp(28px,3.2vw,38px)] font-[300] leading-[1.2] tracking-[-0.015em] text-ink">
          Drawn on the GPU, not in a browser.
        </h2>

        <div className="grid grid-cols-1 gap-5 text-left md:grid-cols-3">
          {cards.map((c) => (
            <article
              key={c.kicker}
              className="flex flex-col rounded-[18px] bg-card p-5 pb-[26px]"
            >
              <p className="mx-1.5 mb-4 font-mono text-[11px] uppercase tracking-[0.12em] text-ink-3">
                {c.kicker}
              </p>
              <div className="flex min-h-[168px] items-center justify-center rounded-xl bg-well px-5 py-6">
                <div className="w-full">{c.art}</div>
              </div>
              <h3 className="mx-1.5 mb-2 mt-[22px] text-[16.5px] font-medium text-ink">
                {c.title}
              </h3>
              <p className="mx-1.5 text-[14px] leading-[1.6] text-ink-2">
                {c.body}
              </p>
            </article>
          ))}
        </div>
      </Band>
    </Shell>
  );
}
