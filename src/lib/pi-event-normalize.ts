/**
 * Make Pi SSE payloads pass `@assistant-ui/react-pi`'s stream validator.
 *
 * Invalid `message_update` events abort the fetch stream and force a snapshot
 * reconnect — the UI then paints the finished reply in one shot. Providers
 * (OpenAI Responses signatures, partial tool-call JSON, missing usage/cost
 * mid-stream) trip that validator "sometimes".
 */

const EMPTY_COST = {
  input: 0,
  output: 0,
  cacheRead: 0,
  cacheWrite: 0,
  total: 0,
};

const EMPTY_USAGE = {
  input: 0,
  output: 0,
  cacheRead: 0,
  cacheWrite: 0,
  totalTokens: 0,
  cost: EMPTY_COST,
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

function asNumber(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function normalizeUsage(value: unknown): typeof EMPTY_USAGE {
  const usage = isRecord(value) ? value : {};
  const cost = isRecord(usage.cost) ? usage.cost : {};
  return {
    input: asNumber(usage.input),
    output: asNumber(usage.output),
    cacheRead: asNumber(usage.cacheRead),
    cacheWrite: asNumber(usage.cacheWrite),
    totalTokens: asNumber(usage.totalTokens),
    cost: {
      input: asNumber(cost.input),
      output: asNumber(cost.output),
      cacheRead: asNumber(cost.cacheRead),
      cacheWrite: asNumber(cost.cacheWrite),
      total: asNumber(cost.total),
    },
  };
}

function normalizeAssistantPart(part: unknown): unknown {
  if (!isRecord(part) || typeof part.type !== "string") return part;
  if (part.type === "text") {
    if (part.textSignature != null && typeof part.textSignature !== "string") {
      const { textSignature: _drop, ...rest } = part;
      return rest;
    }
    return part;
  }
  if (part.type === "toolCall" && typeof part.arguments === "string") {
    try {
      return { ...part, arguments: JSON.parse(part.arguments) as unknown };
    } catch {
      return { ...part, arguments: {} };
    }
  }
  return part;
}

function normalizeAssistantMessage(message: unknown): unknown {
  if (!isRecord(message) || message.role !== "assistant") return message;
  return {
    ...message,
    api: typeof message.api === "string" ? message.api : "unknown",
    provider: typeof message.provider === "string" ? message.provider : "unknown",
    model: typeof message.model === "string" ? message.model : "unknown",
    stopReason:
      typeof message.stopReason === "string" ? message.stopReason : "pending",
    timestamp:
      typeof message.timestamp === "number" ? message.timestamp : Date.now(),
    usage: normalizeUsage(message.usage),
    content: Array.isArray(message.content)
      ? message.content.map(normalizeAssistantPart)
      : message.content,
  };
}

function normalizeAssistantDelta(delta: unknown): unknown {
  if (!isRecord(delta) || typeof delta.type !== "string") return delta;
  const next = { ...delta };
  if ("partial" in next) next.partial = normalizeAssistantMessage(next.partial);
  if ("message" in next) next.message = normalizeAssistantMessage(next.message);
  if ("error" in next) next.error = normalizeAssistantMessage(next.error);
  return next;
}

/** Clone + normalize a supervisor event so the HTTP client will accept it. */
export function normalizeOutgoingPiEvent<T>(event: T): T {
  if (!isRecord(event) || typeof event.type !== "string") return event;
  if (
    event.type !== "message_start" &&
    event.type !== "message_update" &&
    event.type !== "message_end"
  ) {
    return event;
  }
  const next: Record<string, unknown> = { ...event };
  if ("message" in next) next.message = normalizeAssistantMessage(next.message);
  if ("assistantMessageEvent" in next) {
    next.assistantMessageEvent = normalizeAssistantDelta(
      next.assistantMessageEvent,
    );
  }
  return next as T;
}
