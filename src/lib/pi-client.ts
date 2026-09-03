/**
 * Shared Pi SSE client — the browser half of the `@assistant-ui/react-pi`
 * HTTP/SSE transport. Points at `agent/sse-server.ts` (run: `pnpm agent:sse`).
 */

import { createPiHttpClient } from "@assistant-ui/react-pi";

export const SSE_BASE_URL = "http://localhost:8913";

export const piClient = createPiHttpClient({ baseUrl: SSE_BASE_URL });
