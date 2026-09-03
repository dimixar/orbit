// Verify: models list in composer + threads list in sidebar.
import { spawn } from "node:child_process";
import WebSocket from "ws";

const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PORT = 9262;
const APP_URL = "http://localhost:1420/";

const chrome = spawn(CHROME, [
  "--headless=new",
  `--remote-debugging-port=${PORT}`,
  "--no-first-run",
  "--user-data-dir=/tmp/orbit-chrome-profile-models",
  "--window-size=1280,900",
  APP_URL,
], { stdio: "ignore" });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function connect() {
  for (let i = 0; i < 30; i++) {
    try {
      const targets = await (await fetch(`http://localhost:${PORT}/json`)).json();
      const page = targets.find((t) => t.type === "page");
      if (page) return new WebSocket(page.webSocketDebuggerUrl);
    } catch { /* retry */ }
    await sleep(500);
  }
  throw new Error("no chrome");
}

let msgId = 0;
const pending = new Map();
const ws = await connect();
await new Promise((r) => ws.once("open", r));
ws.on("message", (d) => {
  const m = JSON.parse(d.toString());
  if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
  if (m.method === "Runtime.consoleAPICalled" && m.params.type === "error") {
    console.log("CONSOLE ERR:", m.params.args?.map(a => a.value ?? a.description ?? "").join(" ").slice(0, 200));
  }
});
const send = (method, params = {}) => new Promise((res) => {
  const id = ++msgId; pending.set(id, res);
  ws.send(JSON.stringify({ id, method, params }));
});
const ev = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (r.result?.exceptionDetails) return "EXC: " + (r.result.exceptionDetails.exception?.description ?? "");
  return r.result?.result?.value;
};

await send("Page.enable");
await send("Runtime.enable");
await sleep(6000);

// Check the sidebar sessions list.
const sidebar = await ev(`(() => {
  const sidebar = document.querySelector('[class*="sidebar"]');
  const text = sidebar?.innerText ?? '';
  const hasSessions = text.includes('Sessions');
  const sessionCount = (text.match(/\\n/g) ?? []).length;
  return JSON.stringify({ hasSessions, text: text.slice(0, 400) });
})()`);
console.log("SIDEBAR:", sidebar);

// Check the composer model chip.
const composer = await ev(`(() => {
  const modelBtn = document.querySelector('button[aria-label="Model"]');
  return JSON.stringify({ hasModelBtn: !!modelBtn, label: modelBtn?.innerText ?? null });
})()`);
console.log("COMPOSER:", composer);

// Open the model menu and check the list.
await ev(`document.querySelector('button[aria-label="Model"]')?.click()`);
await sleep(800);
const modelMenu = await ev(`(() => {
  const items = [...document.querySelectorAll('[role="menuitem"]')].map(i => i.innerText);
  return JSON.stringify({ items: items.slice(0, 10) });
})()`);
console.log("MODEL MENU:", modelMenu);

chrome.kill();
process.exit(0);
