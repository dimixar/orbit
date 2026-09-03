/**
 * Pi SSE server — exposes the `@assistant-ui/react-pi` PiClient contract over
 * HTTP + SSE, backed by `createPiNodeClient` (which drives the Pi SDK
 * in-process via a process-singleton `PiThreadSupervisor`).
 *
 * This is the server half of the assistant-ui integration:
 *
 *   browser (webview)                    this server (Node)
 *   ─────────────────                    ──────────────────
 *   createPiHttpClient  ──HTTP/SSE──▶    createPiNodeClient
 *   usePiRuntime                          └ PiThreadSupervisor → Pi SDK
 *
 * Run in dev:  pnpm agent:sse
 *
 * NOTE: this process drives the Pi SDK on `PI_WORKSPACE_PATH` (default: the
 * project dir). Don't run it at the same time as the WebSocket daemon
 * (`pnpm agent`) on the same workspace — two processes must not manage the
 * same session files. Use a different `PI_WORKSPACE_PATH` if you need both.
 */
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { createPiNodeClient } from "@assistant-ui/react-pi/node";

const PORT = Number(process.env.PI_SSE_PORT ?? 8913);
const WORKSPACE_PATH = process.env.PI_WORKSPACE_PATH ?? process.cwd();

const client = createPiNodeClient({ workspacePath: WORKSPACE_PATH });

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, PATCH, DELETE, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
};

function readBody(req: IncomingMessage): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("end", () => {
      if (chunks.length === 0) return resolve({});
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString("utf8")));
      } catch (error) {
        reject(error);
      }
    });
    req.on("error", reject);
  });
}

function sendJson(res: ServerResponse, status: number, body: unknown): void {
  res.writeHead(status, {
    "Content-Type": "application/json",
    ...CORS_HEADERS,
  });
  res.end(JSON.stringify(body));
}

function sendNoContent(res: ServerResponse): void {
  res.writeHead(204, CORS_HEADERS);
  res.end();
}

function sendError(res: ServerResponse, error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  console.error("[pi-sse] error:", message);
  sendJson(res, 500, { error: message });
}

/** Wraps a route handler, catching errors into a 500. */
function route(
  handler: (req: IncomingMessage, res: ServerResponse, body: unknown) => Promise<void> | void,
) {
  return async (req: IncomingMessage, res: ServerResponse, body: unknown) => {
    try {
      await handler(req, res, body);
    } catch (error) {
      sendError(res, error);
    }
  };
}

// ---------------------------------------------------------------------------
// SSE stream
// ---------------------------------------------------------------------------

function streamEvents(
  req: IncomingMessage,
  res: ServerResponse,
  threadId: string,
): void {
  const includeSnapshot = new URL(req.url ?? "/", "http://localhost").searchParams.get(
    "snapshot",
  ) !== "false";

  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache, no-transform",
    Connection: "keep-alive",
    "X-Accel-Buffering": "no",
    ...CORS_HEADERS,
  });
  res.write(": connected\n\n");

  const unsubscribe = client.subscribe(
    threadId,
    (event) => {
      res.write(`data: ${JSON.stringify(event)}\n\n`);
    },
    { includeSnapshot },
  );

  // Heartbeat so proxies don't drop the idle stream.
  const heartbeat = setInterval(() => res.write(": ping\n\n"), 15000);

  req.on("close", () => {
    clearInterval(heartbeat);
    unsubscribe();
  });
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

const server = createServer(async (req, res) => {
  if (req.method === "OPTIONS") {
    res.writeHead(204, CORS_HEADERS);
    res.end();
    return;
  }

  const url = new URL(req.url ?? "/", "http://localhost");
  const path = url.pathname;
  const method = req.method ?? "GET";
  const body = await readBody(req).catch(() => ({}));

  // GET /threads
  if (method === "GET" && path === "/threads") {
    return route(async (_req, res) => {
      const threads = await client.listThreads({
        workspacePath: url.searchParams.get("workspacePath") ?? undefined,
        includeArchived: url.searchParams.get("includeArchived") === "true",
      });
      sendJson(res, 200, threads);
    })(req, res, body);
  }

  // POST /threads
  if (method === "POST" && path === "/threads") {
    return route(async (_req, res, body) => {
      const snapshot = await client.createThread(
        (body ?? {}) as { workspacePath?: string; title?: string; initialMessage?: unknown },
      );
      sendJson(res, 200, snapshot);
    })(req, res, body);
  }

  // GET /models
  if (method === "GET" && path === "/models") {
    return route(async (_req, res) => {
      const models = await client.getAvailableModels({
        workspacePath: url.searchParams.get("workspacePath") ?? undefined,
      });
      sendJson(res, 200, models);
    })(req, res, body);
  }

  // /threads/:id[/action[/subaction]]
  const match = path.match(/^\/threads\/([^/]+)(?:\/([^/]+))?(?:\/([^/]+))?$/);
  if (match) {
    const threadId = decodeURIComponent(match[1]!);
    const action = match[2];
    const subaction = match[3];

    // GET /threads/:id
    if (method === "GET" && !action) {
      return route(async (_req, res) => {
        sendJson(res, 200, await client.getThread(threadId));
      })(req, res, body);
    }

    // PATCH /threads/:id  (rename)
    if (method === "PATCH" && !action) {
      return route(async (_req, res, body) => {
        const title = (body as { title?: string } | undefined)?.title;
        if (typeof title !== "string") throw new Error("PATCH /threads/:id requires { title }");
        await client.renameThread(threadId, title);
        sendNoContent(res);
      })(req, res, body);
    }

    // DELETE /threads/:id
    if (method === "DELETE" && !action) {
      return route(async (_req, res) => {
        await client.deleteThread(threadId);
        sendNoContent(res);
      })(req, res, body);
    }

    if (action) {
      // GET /threads/:id/events  (SSE)
      if (method === "GET" && action === "events") {
        return streamEvents(req, res, threadId);
      }

      // POST /threads/:id/messages
      if (method === "POST" && action === "messages") {
        return route(async (_req, res, body) => {
          const input = (body as { input?: unknown } | undefined)?.input;
          if (input === undefined) throw new Error("POST /messages requires { input }");
          await client.sendMessage(threadId, input as never);
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/cancel
      if (method === "POST" && action === "cancel") {
        return route(async (_req, res) => {
          await client.cancelRun(threadId);
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/queue/clear
      if (method === "POST" && action === "queue" && subaction === "clear") {
        return route(async (_req, res) => {
          sendJson(res, 200, await client.clearQueue(threadId));
        })(req, res, body);
      }

      // POST /threads/:id/model
      if (method === "POST" && action === "model") {
        return route(async (_req, res, body) => {
          const { provider, modelId } = (body ?? {}) as { provider?: string; modelId?: string };
          if (!provider || !modelId) throw new Error("POST /model requires { provider, modelId }");
          await client.setModel(threadId, { provider, modelId });
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/thinking
      if (method === "POST" && action === "thinking") {
        return route(async (_req, res, body) => {
          const level = (body as { level?: string } | undefined)?.level;
          if (!level) throw new Error("POST /thinking requires { level }");
          await client.setThinkingLevel(threadId, level as never);
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/archive | /unarchive
      if (method === "POST" && (action === "archive" || action === "unarchive")) {
        return route(async (_req, res) => {
          if (action === "archive") await client.archiveThread(threadId);
          else await client.unarchiveThread(threadId);
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/host-ui
      if (method === "POST" && action === "host-ui") {
        return route(async (_req, res, body) => {
          const response = (body as { response?: unknown } | undefined)?.response;
          if (response === undefined) throw new Error("POST /host-ui requires { response }");
          await client.respondToHostUiRequest(threadId, response as never);
          sendNoContent(res);
        })(req, res, body);
      }
    }
  }

  sendJson(res, 404, { error: `Not found: ${method} ${path}` });
});

server.listen(PORT, () => {
  console.log(`[pi-sse] listening on http://localhost:${PORT}`);
  console.log(`[pi-sse] workspace: ${WORKSPACE_PATH}`);
  console.log(`[pi-sse] PiClient contract: GET/POST /threads, GET /threads/:id/events (SSE)`);
});
