import { describe, expect, it } from "vitest";
import { isSessionRunning } from "@/lib/session-running";

describe("isSessionRunning", () => {
  it("uses catalog and thread-item status", () => {
    expect(isSessionRunning({ metaStatus: "running" })).toBe(true);
    expect(isSessionRunning({ itemStatus: "running" })).toBe(true);
    expect(isSessionRunning({ metaStatus: "idle", itemStatus: "idle" })).toBe(
      false,
    );
  });

  it("marks the open thread from live extras even when the catalog is idle", () => {
    expect(
      isSessionRunning({
        metaStatus: "idle",
        isActive: true,
        extrasStatus: "running",
      }),
    ).toBe(true);
    expect(
      isSessionRunning({
        metaStatus: "idle",
        isActive: false,
        extrasStatus: "running",
      }),
    ).toBe(false);
  });
});
