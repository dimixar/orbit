import { describe, expect, it } from "vitest";
import {
  includeSnapshotFromUrl,
  isRunSettledEvent,
  liveStatusFromRecords,
  shouldIncludeSnapshot,
} from "../../../agent/sse-stream";

describe("liveStatusFromRecords", () => {
  it("reports running from the live supervisor record", () => {
    const records = new Map([
      [
        "sess-1",
        { session: { sessionId: "sess-1", isStreaming: true } },
      ],
    ]);
    expect(liveStatusFromRecords("sess-1", records)).toBe("running");
    expect(liveStatusFromRecords("missing", records)).toBeUndefined();
  });

  it("resolves a catalog id that differs from the map key", () => {
    const records = new Map([
      [
        "internal",
        { threadId: "catalog-id", session: { isCompacting: true } },
      ],
    ]);
    expect(liveStatusFromRecords("catalog-id", records)).toBe("running");
  });
});

describe("includeSnapshotFromUrl", () => {
  it("honors the official ?snapshot=false skip", () => {
    expect(
      includeSnapshotFromUrl(
        new URL("http://localhost/threads/t1/events?snapshot=false"),
      ),
    ).toBe(false);
    expect(
      includeSnapshotFromUrl(new URL("http://localhost/threads/t1/events")),
    ).toBe(true);
  });
});

describe("shouldIncludeSnapshot", () => {
  it("always snapshots when the client asked for one", () => {
    expect(shouldIncludeSnapshot(true, "running")).toBe(true);
    expect(shouldIncludeSnapshot(true, "idle")).toBe(true);
  });

  it("still snapshots idle/unknown threads when the client skipped", () => {
    expect(shouldIncludeSnapshot(false, "idle")).toBe(true);
    expect(shouldIncludeSnapshot(false, undefined)).toBe(true);
  });

  it("skips a mid-run snapshot when the client already has state", () => {
    expect(shouldIncludeSnapshot(false, "running")).toBe(false);
  });
});

describe("isRunSettledEvent", () => {
  it("treats agent_settled and a finished agent_end as settled", () => {
    expect(isRunSettledEvent({ type: "agent_settled" })).toBe(true);
    expect(isRunSettledEvent({ type: "agent_end" })).toBe(true);
    expect(isRunSettledEvent({ type: "agent_end", willRetry: false })).toBe(
      true,
    );
  });

  it("does not settle a retrying agent_end or streaming deltas", () => {
    expect(isRunSettledEvent({ type: "agent_end", willRetry: true })).toBe(
      false,
    );
    expect(isRunSettledEvent({ type: "message_update" })).toBe(false);
  });
});
