/**
 * Orbit auto-title (pi extension).
 *
 * Orbit loads this file with `pi --extension <path>` on every session it
 * spawns. After a session's first turn settles, it asks a model for a short
 * title from the first exchange and sets it with `pi.setSessionName`. pi
 * persists a `session_info` entry and emits `session_info_changed`, so Orbit
 * shows the title live and it survives reloads.
 *
 * The setting lives in `~/.orbit-pi/auto-title.json` and is read when the
 * session's first turn settles, so a change in Orbit reaches sessions started
 * afterwards (and any session whose first turn has not settled yet). The
 * model is Orbit's choice (`{provider, id}`) or, by default, the session's
 * active model. The pure pieces are in `title.js`.
 *
 * Failure is always quiet: a title is a nicety, never a reason to disturb a
 * run. Only a session's very first exchange titles it — a resumed session
 * (one that already had prompts before Orbit opened it) is left with its
 * existing name/first message.
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";

import {
  CONFIG_FILE,
  buildTitlePrompt,
  firstExchange,
  isStaleContext,
  isTitleable,
  parseConfig,
  resolveModel,
  titleFromResponse,
} from "./title.js";

const MAX_TITLE_TOKENS = 64;

/** Config path resolved per call, so a changed `HOME` (tests) is honored. */
function configPath() {
  return path.join(os.homedir(), ".orbit-pi", CONFIG_FILE);
}

/** In-flight title request; Orbit's tests await it to stay deterministic. */
let inflight = Promise.resolve();

export function pendingTitle() {
  return inflight;
}

/** Read Orbit's setting; a missing/broken file means "on, active model". */
function readConfig() {
  try {
    return parseConfig(fs.readFileSync(configPath(), "utf8"));
  } catch {
    return parseConfig(null);
  }
}

/** Generate and set the title. Never throws. Returns true when no further
 * attempt is worth making (titled, deliberately skipped, or the session went
 * stale); false when the ask failed transiently and the next settle should
 * retry. */
async function generate(pi, ctx) {
  const config = readConfig();
  if (!config.enabled) {
    return true;
  }
  // A session the user has already named (or a previous run titled) is done.
  if (pi.getSessionName()) {
    return true;
  }
  const { user, assistant, userCount } = firstExchange(ctx.sessionManager?.getBranch?.() ?? []);
  // Only the very first exchange titles: a resumed session (or one on its
  // second prompt) is left with its existing name/first message. A tool-using
  // turn streams several assistant messages, so user prompts — not replies —
  // are the fresh-vs-resumed signal.
  if (userCount !== 1 || !isTitleable(user, assistant)) {
    return true;
  }
  const model = resolveModel(ctx, config);
  if (!model) {
    return true;
  }
  try {
    const response = await ctx.modelRegistry.complete(
      model,
      {
        messages: [
          {
            role: "user",
            content: [{ type: "text", text: buildTitlePrompt(user, assistant) }],
            timestamp: Date.now(),
          },
        ],
      },
      {
        maxTokens: MAX_TITLE_TOKENS,
        cacheRetention: "none",
        sessionId: randomUUID(),
      },
    );
    const title = titleFromResponse(response);
    // Never clobber a name the user set while the request was in flight.
    if (title && !pi.getSessionName()) {
      pi.setSessionName(title);
    }
    return Boolean(title);
  } catch (error) {
    if (isStaleContext(error)) {
      return true;
    }
    // Quiet by design: titling must never surface as a session error.
    return false;
  }
}

export default function activate(pi) {
  let considered = false;

  pi.on("agent_settled", (_event, ctx) => {
    if (considered) {
      return;
    }
    considered = true;
    // pi awaits async lifecycle handlers, and the title is not worth delaying
    // the settled state — kick it off detached. Orbit reads the resulting
    // `session_info_changed` from the event stream. A transient failure
    // un-arms the guard so the next settle asks again.
    inflight = generate(pi, ctx)
      .then((done) => {
        if (!done) {
          considered = false;
        }
      })
      .catch(() => {
        considered = false;
      });
  });
}
