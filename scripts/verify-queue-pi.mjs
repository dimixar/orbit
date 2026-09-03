// Verify send→stop + queue with a tool-triggering prompt (longer run).
import { spawn } from "node:child_process";
import WebSocket from "ws";

const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PORT = 9249;
const APP_URL = "http://localhost:1420/#pi-demo";

const chrome = spawn(CHROME, [
  "--headless=new",
  `--remote-debugging-port=${PORT}`,
  "--no-first-run",
  "--user-data-dir=/tmp/orbit-chrome-profile-queue5",
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
await sleep(5000);

const buttons = () => ev(`(() => {
  const send = document.querySelector('button[aria-label="Send"]');
  const stop = document.querySelector('button[aria-label="Stop generation"]');
  const queued = [...document.querySelectorAll('span')].filter(s => s.textContent === 'queued').length;
  return JSON.stringify({ send: !!send, stop: !!stop, queued });
})()`);

// Send a tool-triggering prompt (takes longer).
await ev(`(() => {
  const ta = document.querySelector('textarea[aria-label="Prompt"]');
  const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value').set;
  setter.call(ta, 'Explore the src directory of this project and list the main components with a one-line description of each');
  ta.dispatchEvent(new Event('input', { bubbles: true }));
})()`);
await sleep(200);
await ev(`document.querySelector('button[aria-label="Send"]')?.click()`);

// Poll fast for the stop button.
let stopSeen = false;
for (let i = 0; i < 30; i++) {
  await sleep(200);
  const b = JSON.parse(await buttons());
  if (b.stop) { stopSeen = true; console.log("STOP button at t+" + ((i + 1) * 0.2).toFixed(1) + "s"); break; }
}
if (!stopSeen) console.log("STOP never appeared in 6s");

// While running, queue a follow-up via real keypress.
await ev(`(() => {
  const ta = document.querySelector('textarea[aria-label="Prompt"]');
  const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value').set;
  setter.call(ta, 'and also list the lib directory');
  ta.dispatchEvent(new Event('input', { bubbles: true }));
  ta.focus();
})()`);
await sleep(300);
await send("Input.dispatchKeyEvent", { type: "char", key: "Enter", code: "Enter", text: "\r", unmodifiedText: "\r" });
await sleep(2000);
console.log("after follow-up:", await buttons());

// Watch the queue process.
for (const [label, delay] of [["t+15s", 13000], ["t+35s", 20000]]) {
  await sleep(delay);
  console.log(label + ":", await buttons());
}

chrome.kill();
process.exit(0);
