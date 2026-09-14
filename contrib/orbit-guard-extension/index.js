/**
 * Orbit's access-mode guard (pi extension).
 *
 * Orbit loads this file with `pi --extension <path>` on every session it
 * spawns. It hooks pi's `tool_call` event — which runs after a tool call is
 * proposed and can block it — and, for any mutating call the active access
 * mode does not auto-approve, asks the user through `ctx.ui.confirm()`. In
 * RPC mode that confirm becomes an `extension_ui_request`; Orbit renders it
 * natively and answers it over stdin.
 *
 * The mode lives in `~/.orbit-pi/access.json` and is read fresh on every tool
 * call, so a change in Orbit re-arms live sessions with no restart. See
 * `policy.js` for the decision table and `crates/orbit-pi/src/access.rs` for
 * the writer.
 *
 * Fail-safe: if no UI is available to confirm (print/json mode) or the prompt
 * throws, the call is blocked rather than allowed.
 */
import { decide, readMode, summarize } from "./policy.js";

/** Title shown on the confirmation dialog. */
const CONFIRM_TITLE = "Allow this action?";

/**
 * Confirms are serialized: pi can preflight several sibling tool calls from
 * one assistant message, and Orbit renders one modal at a time. Chaining the
 * prompts guarantees each is answered before the next is shown, instead of a
 * second request arriving while the first is open and being auto-cancelled.
 */
let confirmChain = Promise.resolve();

function serializeConfirm(run) {
  const result = confirmChain.then(run, run);
  // Keep the chain alive regardless of how the individual confirm settles.
  confirmChain = result.then(
    () => undefined,
    () => undefined,
  );
  return result;
}

export default function activate(pi) {
  pi.on("tool_call", async (event, ctx) => {
    const decision = decide(readMode(), event.toolName);
    if (decision === "allow") return;

    // No UI (print / json mode): there is no way to ask, so deny rather than
    // silently run a mutating tool the current mode wants confirmed.
    if (!ctx?.hasUI) {
      return { block: true, reason: "Blocked: Orbit has no UI to confirm this action" };
    }

    let allowed = false;
    try {
      allowed = await serializeConfirm(() =>
        ctx.ui.confirm(CONFIRM_TITLE, summarize(event.toolName, event.input)),
      );
    } catch {
      allowed = false;
    }
    if (!allowed) return { block: true, reason: "Denied in Orbit" };
  });
}
