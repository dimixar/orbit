import type { TimelineStep, StepState } from "../types/timeline";

function calculateDiffStatsFromPatch(
  patches: Array<{ lines?: string[] }>,
): string | undefined {
  let addedLines = 0;
  let removedLines = 0;

  for (const patch of patches) {
    if (!patch.lines) continue;
    for (const line of patch.lines) {
      if (line.startsWith("+")) addedLines++;
      else if (line.startsWith("-")) removedLines++;
    }
  }

  if (addedLines === 0 && removedLines === 0) return undefined;

  const parts: string[] = [];
  if (addedLines > 0) parts.push(`+${addedLines}`);
  if (removedLines > 0) parts.push(`-${removedLines}`);
  return parts.join(" ");
}

function getDiffLinesFromPatch(
  patches: Array<{ lines?: string[] }>,
): { type: "add" | "remove" | "context"; content: string }[] {
  const result: { type: "add" | "remove" | "context"; content: string }[] = [];

  for (const patch of patches) {
    if (!patch.lines) continue;
    for (const line of patch.lines) {
      if (line.startsWith("+")) {
        result.push({ type: "add", content: line.slice(1) });
      } else if (line.startsWith("-")) {
        result.push({ type: "remove", content: line.slice(1) });
      } else if (line.startsWith(" ")) {
        result.push({ type: "context", content: line.slice(1) });
      }
    }
  }

  return result;
}

/** Parse a unified diff string (pi's `details.patch`) into typed lines.
 *  Only lines after the first `@@` hunk header are considered, so file
 *  headers (---/+++/===) never leak into the diff body. */
function parseUnifiedPatchLines(
  patch: string,
): { type: "add" | "remove" | "context"; content: string }[] {
  const result: { type: "add" | "remove" | "context"; content: string }[] = [];
  let inHunk = false;

  for (const line of patch.split("\n")) {
    if (!inHunk) {
      if (line.startsWith("@@")) inHunk = true;
      continue;
    }
    if (line.startsWith("+")) {
      result.push({ type: "add", content: line.slice(1) });
    } else if (line.startsWith("-")) {
      result.push({ type: "remove", content: line.slice(1) });
    } else if (line.startsWith(" ")) {
      result.push({ type: "context", content: line.slice(1) });
    }
    // "\\ No newline at end of file" markers carry no displayable line.
  }

  return result;
}

export function mapToolStateToStepState(
  aiState: "partial-call" | "call" | "result",
): StepState {
  return aiState === "result" ? "complete" : "animating";
}

export function mapToolNameToVariant(
  toolName: string,
): "thinking" | "action" | "search" | undefined {
  const lower = toolName.toLowerCase();
  if (lower === "thinking" || lower === "reasoning") return "thinking";
  if (
    lower === "websearch" ||
    lower === "web_search" ||
    lower === "grep" ||
    lower === "glob" ||
    lower === "webfetch" ||
    lower === "web_fetch"
  )
    return "search";
  return undefined;
}

function extractToolDetail(
  toolName: string,
  args: Record<string, any>,
): string {
  switch (toolName) {
    case "Bash":
      return args?.command ? String(args.command).slice(0, 80) : "";
    case "Edit":
    case "Write":
    case "Read":
      // pi sends `path`; keep `file_path` for non-pi runtimes.
      return args?.path ?? args?.file_path
        ? (String(args.path ?? args.file_path).split("/").pop() ?? "")
        : "";
    case "Grep":
      return args?.pattern ? String(args.pattern) : "";
    case "Glob":
      return args?.pattern ? String(args.pattern) : "";
    case "WebSearch":
    case "web_search":
      return args?.query ? String(args.query) : "";
    case "WebFetch":
    case "web_fetch":
      return args?.url ? String(args.url).slice(0, 60) : "";
    default:
      return "";
  }
}

export function mapToolInvocationToStep(
  toolCallId: string,
  toolInvocation: {
    toolName: string;
    args?: Record<string, any>;
    state: "partial-call" | "call" | "result";
    result?: any;
  },
): Extract<TimelineStep, { type: "tool-call" }> {
  const { toolName, args = {}, result } = toolInvocation;
  const displayToolName =
    toolName === "PlanWrite"
      ? "Plan"
      : toolName === "TodoWrite"
        ? "Todo"
        : toolName;
  const detail = extractToolDetail(toolName, args);

  const step: Extract<TimelineStep, { type: "tool-call" }> = {
    id: toolCallId,
    type: "tool-call",
    toolName: displayToolName,
    toolDetail: detail,
    duration: Number.MAX_SAFE_INTEGER,
    toolVariant: mapToolNameToVariant(toolName),
  };

  if (toolName === "Bash") {
    step.bashCommand = args?.command ? String(args.command) : undefined;
    if (toolInvocation.state === "result" && result) {
      if (typeof result === "string") {
        step.bashOutput = result;
        step.bashSuccess = true;
      } else if (typeof result === "object") {
        const stdout =
          typeof result?.stdout === "string"
            ? result.stdout
            : typeof result?.output === "string"
              ? result.output
              : "";
        const stderr = typeof result?.stderr === "string" ? result.stderr : "";
        step.bashOutput = [stdout, stderr]
          .filter(Boolean)
          .join(stdout && stderr ? "\n" : "");
        const exitCode = result?.exitCode ?? result?.exit_code;
        step.bashSuccess = exitCode === undefined ? true : exitCode === 0;
      } else {
        step.bashOutput = JSON.stringify(result);
        step.bashSuccess = true;
      }
    }
  }

  if (toolName === "Edit" || toolName === "Write" || toolName === "Read") {
    // pi sends `path`; `file_path` covers non-pi runtimes.
    const rawPath = args?.path ?? args?.file_path;
    step.filePath = rawPath ? String(rawPath) : undefined;
  }

  if (toolName === "Write") {
    const content =
      typeof result?.content === "string"
        ? result.content
        : typeof args?.content === "string"
          ? args.content
          : "";

    if (content) {
      const lines = content.split("\n");
      step.diffStats = `+${lines.length}`;
      step.diffLines = lines.map((line: string) => ({
        type: "add",
        content: line,
      }));
    }
  }

  if (toolName === "Edit") {
    if (typeof result?.details?.patch === "string" && result.details.patch) {
      // pi returns a unified diff string — parse it for the card body and
      // the +N/-N header stats.
      step.diffLines = parseUnifiedPatchLines(result.details.patch);
      const added = step.diffLines.filter((l) => l.type === "add").length;
      const removed = step.diffLines.filter((l) => l.type === "remove").length;
      if (added > 0 || removed > 0) {
        step.diffStats = [added > 0 ? `+${added}` : "", removed > 0 ? `-${removed}` : ""]
          .filter(Boolean)
          .join(" ");
      }
    } else if (Array.isArray(result?.structuredPatch)) {
      step.diffStats = calculateDiffStatsFromPatch(result.structuredPatch);
      step.diffLines = getDiffLinesFromPatch(result.structuredPatch);
    }
  }

  if (
    toolName === "WebSearch" ||
    toolName === "web_search" ||
    toolName === "Grep" ||
    toolName === "Glob"
  ) {
    step.searchQuery =
      (args?.query ?? args?.pattern)
        ? String(args?.query ?? args?.pattern)
        : undefined;
    step.searchSource =
      toolName === "WebSearch" || toolName === "web_search" ? "web" : "code";
  }

  if (
    toolName.toLowerCase() === "thinking" ||
    toolName.toLowerCase() === "reasoning"
  ) {
    step.thoughtContent =
      typeof args?.thought === "string"
        ? args.thought
        : typeof result === "string"
          ? result
          : undefined;
  }

  return step;
}
