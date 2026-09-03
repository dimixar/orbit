// Capture console errors + check if models load.
import { spawn } from "node:child_process";
import WebSocket from "ws";

const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PORT = 9265;
const APP_URL = "http://localhost:1420/";

const chrome = spawn(CHROME, [
  "--headless=new",
  `--remote-debugging-port=${PORT}`,
  "--no-first-run",
  "--user-data-dir=/tmp/orbit-chrome-profile-models4",
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
  if (m.method === "Runtime.consoleAPICalled") {
    const text = m.params.args?.map(a => a.value ?? a.description ?? "").join(" ");
    if (m.params.type === "error" || m.params.type === "warning") {
      console.log(`[${m.params.type}]`, text.slice(0, 300));
    }
  }
  if (m.method === "Runtime.exceptionThrown") {
    console.log("[EXC]", m.params.exceptionDetails?.exception?.description?.slice(0, 300));
  }
  if (m.method === "Network.requestWillBeSent" && m.params.request.url.includes("8913")) {
    console.log("REQ:", m.params.request.method, m.params.request.url.replace("http://localhost:8913", ""));
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
await send("Network.enable");
await sleep(6000);

const hasModels = await ev(`document.body.innerText.includes('glm-5.2') ? 'YES' : 'NO'`);
console.log("models in DOM:", hasModels);

chrome.kill();
process.exit(0);
