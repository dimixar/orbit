import { describe, expect, it } from "vitest";
import { isOrbitSseHealth } from "@/lib/pi-client";

describe("isOrbitSseHealth", () => {
  it("accepts the orbit SSE identity payload", () => {
    expect(
      isOrbitSseHealth({
        id: "orbit-pi-sse",
        ok: true,
        pid: 12,
      }),
    ).toBe(true);
  });

  it("rejects other listeners on the same port", () => {
    expect(isOrbitSseHealth({ id: "other", ok: true })).toBe(false);
    expect(isOrbitSseHealth({ id: "orbit-pi-sse", ok: false })).toBe(false);
    expect(isOrbitSseHealth(null)).toBe(false);
  });
});
