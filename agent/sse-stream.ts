import type { ServerResponse } from "node:http";
import { normalizeOutgoingPiEvent } from "../src/lib/pi-event-normalize";

export type LiveSessionLike = {
  threadId?: string;
  session?: {
    sessionId?: string;
    isStreaming?: boolean;
    isCompacting?: boolean;
    isRetrying?: boolean;
  };
};

/** Official HTTP contract: `?snapshot=false` skips the initial snapshot. */
export function includeSnapshotFromUrl(url: URL): boolean {
  return url.searchParams.get("snapshot") !== "false";
}

/**
 * Whether the SSE subscribe should start with an authoritative snapshot.
 *
 * The SDK's custom-UI pattern is subscribe-then-prompt; react-pi then opens
 * with `?snapshot=false` when it already has `getThread`. Honor that while a
 * run is live so a reconnect snapshot does not wipe `streamingMessage`. Still
 * snapshot idle/unknown threads so a late subscriber gets `session.messages`.
 */
export function shouldIncludeSnapshot(
  requested: boolean,
  liveStatus: "running" | "idle" | undefined,
): boolean {
  if (requested) return true;
  return liveStatus !== "running";
}

/** SDK: `agent_end.messages` is the completed turn; `agent_settled` means idle. */
export function isRunSettledEvent(event: unknown): boolean {
  if (!event || typeof event !== "object") return false;
  const type = Reflect.get(event, "type");
  if (type === "agent_settled") return true;
  if (type === "agent_end") {
    return Reflect.get(event, "willRetry") !== true;
  }
  return false;
}

export function liveStatusFromRecords(
  threadId: string,
  records: Map<string, LiveSessionLike> | undefined,
): "running" | "idle" | undefined {
  if (!records) return undefined;
  let record = records.get(threadId);
  if (!record) {
    for (const candidate of records.values()) {
      if (
        candidate.threadId === threadId ||
        candidate.session?.sessionId === threadId
      ) {
        record = candidate;
        break;
      }
    }
  }
  if (!record?.session) return undefined;
  const session = record.session;
  if (session.isStreaming || session.isCompacting || session.isRetrying) {
    return "running";
  }
  return "idle";
}

/** Write one SSE frame and push it onto the socket immediately. */
export function writeSseEvent(res: ServerResponse, event: unknown): void {
  const payload = `data: ${JSON.stringify(normalizeOutgoingPiEvent(event))}\n\n`;
  res.socket?.setNoDelay(true);
  res.write(payload);
  const flushable = res as ServerResponse & { flush?: () => void };
  if (typeof flushable.flush === "function") {
    flushable.flush();
  } else {
    res.socket?.uncork();
  }
}
