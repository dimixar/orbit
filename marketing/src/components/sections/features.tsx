import type { ReactNode } from "react";
import { Card } from "@/components/ui/card";
import { Band, Eyebrow, Shell } from "@/components/ui";
import {
  ComposerMock,
  FindMock,
  SessionsMock,
  ToolsMock,
  TranscriptMock,
  WorkbenchMock,
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
    body: "A GPU-rendered list whose cost is independent of message count, with coalesced commits, GFM markdown, and paint-only syntax highlighting.",
    art: <TranscriptMock />,
  },
  {
    kicker: "Composer",
    title: "Prompt, steer, queue, cancel.",
    body: "Model and thinking-effort chips, follow-up queueing, mid-run steering, attachments, and @-mention autocomplete — all against the live pi process.",
    art: <ComposerMock />,
  },
  {
    kicker: "Tools",
    title: "Every tool call, rendered.",
    body: "Bash, edit, read, and thinking rows drawn natively, expanding into argument and output cards with per-section copy and inline edit diffs.",
    art: <ToolsMock />,
  },
  {
    kicker: "Sessions",
    title: "Sessions live where pi put them.",
    body: "Your list is read from pi and grouped by project. Reopen, clone, or hide a workspace without touching pi's sessions — recent ones stay warm.",
    art: <SessionsMock />,
  },
  {
    kicker: "Find",
    title: "Find anything, anywhere.",
    body: "⌘P opens a command palette over sessions, commands, and settings; ⌘F searches the transcript with live counts and jumps; images open in a lightbox.",
    art: <FindMock />,
  },
  {
    kicker: "Workbench",
    title: "The whole workbench, wired to pi.",
    body: "Usage analytics, skills, plugins, models, providers, and settings — including themes, fonts, density, and reduce-motion under Appearance.",
    art: <WorkbenchMock />,
  },
];

export function Features() {
  return (
    <Shell>
      <Band id="features" dashed className="scroll-mt-20 py-16 text-center sm:py-20">
        <Eyebrow className="mb-3.5">The core loop</Eyebrow>
        <h2 className="mx-auto mb-10 max-w-[20ch] text-[clamp(28px,3.2vw,38px)] font-[300] leading-[1.2] tracking-[-0.015em] text-ink">
          The whole loop, in one window.
        </h2>

        <div className="grid grid-cols-1 gap-5 text-left md:grid-cols-3">
          {cards.map((c) => (
            <Card
              key={c.kicker}
              className="gap-4 rounded-[18px] p-5 pb-[26px] ring-0"
            >
              <p className="mx-1.5 font-mono text-[11px] uppercase tracking-[0.12em] text-ink-3">
                {c.kicker}
              </p>
              <div className="mx-1.5 flex min-h-[168px] items-center justify-center rounded-xl bg-well px-5 py-6">
                <div className="w-full">{c.art}</div>
              </div>
              <div className="mx-1.5">
                <h3 className="mb-2 text-[16.5px] font-medium text-ink">
                  {c.title}
                </h3>
                <p className="text-[14px] leading-[1.6] text-ink-2">
                  {c.body}
                </p>
              </div>
            </Card>
          ))}
        </div>
      </Band>
    </Shell>
  );
}
