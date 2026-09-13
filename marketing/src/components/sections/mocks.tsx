import { Brain, CaretRight, Check } from "@phosphor-icons/react/dist/ssr";
import type { ReactNode } from "react";

export function Panel({
  title,
  meta,
  children,
  className = "",
}: {
  title: string;
  meta?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={`overflow-hidden rounded-xl bg-well shadow-[inset_0_0_0_1px_#ffffff0f] ${className}`}
    >
      <div className="flex items-center gap-2 px-4 py-2.5">
        <span className="font-mono text-[11px] text-ink-3">{title}</span>
        {meta ? (
          <span className="ml-auto font-mono text-[10.5px] text-ink-3">
            {meta}
          </span>
        ) : null}
      </div>
      <div className="px-4 pb-4">{children}</div>
    </div>
  );
}

export function TranscriptMock() {
  return (
    <div className="flex flex-col gap-3 text-[11.5px] leading-[1.55]">
      <p className="self-end max-w-[80%] rounded-[9px] bg-slab px-3 py-1.5 text-ink">
        Port the theme switcher.
      </p>
      <div className="flex items-center gap-2 text-ink-3">
        <Brain className="size-3.5" />
        <span className="font-mono text-[10.5px]">thinking · 8.1s</span>
      </div>
      <div className="flex items-center gap-2 rounded-md bg-[#ffffff0a] px-2.5 py-1.5 font-mono text-[10.5px] text-ink-2">
        Run cargo test -p orbit-pi
        <Check className="size-3 text-ink-3" />
      </div>
      <p className="text-ink-2">
        Ported the switcher; 412 tests pass. Restyling the controls now
        <span className="caret" />
      </p>
    </div>
  );
}

export function SessionsMock() {
  const groups = [
    {
      project: "orbit",
      sessions: [
        { title: "Port the theme switcher", meta: "now", active: true },
        { title: "Review RPC settle timing", meta: "3h" },
      ],
    },
    {
      project: "sample-store",
      sessions: [
        { title: "Flag checkout risks", meta: "yd" },
        { title: "Map the codebase", meta: "2d" },
      ],
    },
  ];
  return (
    <div className="flex flex-col gap-3 text-[11.5px]">
      {groups.map((g) => (
        <div key={g.project} className="flex flex-col gap-1">
          <span className="font-mono text-[10px] uppercase tracking-[0.08em] text-ink-3">
            {g.project}
          </span>
          {g.sessions.map((s) => (
            <div
              key={s.title}
              className={`flex items-center gap-2 rounded-[5px] px-2 py-1 ${
                s.active ? "bg-[#ffffff12] text-ink" : "text-ink-2"
              }`}
            >
              <span
                className={`size-[5px] rounded-full ${
                  s.active ? "bg-accent" : "bg-ink-3"
                }`}
              />
              <span className="truncate">{s.title}</span>
              <span className="ml-auto font-mono text-[10px] text-ink-3">
                {s.meta}
              </span>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

export function ToolsMock() {
  const tools = [
    { verb: "Edit", target: "settings.rs", meta: "1.2s", open: true },
    { verb: "Bash", target: "cargo test", meta: "41s" },
    { verb: "Read", target: "auth/session.ts", meta: "0.4s" },
  ];
  return (
    <div className="flex flex-col gap-2 font-mono text-[10.5px]">
      {tools.map((t) => (
        <div key={t.target} className="flex flex-col gap-1.5">
          <div className="flex items-baseline gap-2">
            <CaretRight
              className={`size-3 text-ink-3 ${t.open ? "rotate-90" : ""}`}
            />
            <span className="text-ink-2">{t.verb}</span>
            <span className="truncate text-ink-3">{t.target}</span>
            <span className="ml-auto text-ink-3">{t.meta}</span>
          </div>
          {t.open ? (
            <div className="ml-4 rounded-md bg-[#ffffff0a] px-2.5 py-1.5 text-ink-2">
              {"{ density: 1.0, motion: false }"}
            </div>
          ) : null}
        </div>
      ))}
    </div>
  );
}
