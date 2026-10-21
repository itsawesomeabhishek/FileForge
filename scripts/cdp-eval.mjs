// Dev helper: evaluates a JS expression inside the running FileForge webview via
// the WebView2 remote debugging port (start the app with
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222).
//
//   node scripts/cdp-eval.mjs "<expression>"          (main window)
//   CDP_TARGET=wheel node scripts/cdp-eval.mjs "..."   (wheel overlay)
//   node scripts/cdp-eval.mjs --screenshot out.png
import { readFileSync, writeFileSync } from "node:fs";

const port = process.env.CDP_PORT ?? "9222";
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
// CDP_TARGET=wheel selects the wheel overlay (its document title is "fileforge-wheel").
const wanted = process.env.CDP_TARGET === "wheel" ? "fileforge-wheel" : null;
const pages = targets.filter((t) => t.type === "page");
const page = wanted
  ? pages.find((t) => t.title === wanted)
  : (pages.find((t) => t.url.includes("localhost:1420") && t.title !== "fileforge-wheel") ?? pages.find((t) => t.title !== "fileforge-wheel"));
if (!page) throw new Error("no page target found");

const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  socket.onopen = resolve;
  socket.onerror = reject;
});

let nextId = 1;
const pending = new Map();
socket.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.id && pending.has(message.id)) {
    pending.get(message.id)(message);
    pending.delete(message.id);
  }
};
const send = (method, params = {}) =>
  new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, resolve);
    socket.send(JSON.stringify({ id, method, params }));
  });

const args = process.argv.slice(2);

// CDP_WHEN="<expr>" polls until the expression is truthy (up to 40 s) before acting.
if (process.env.CDP_WHEN) {
  const deadline = Date.now() + 40000;
  for (;;) {
    const probe = await send("Runtime.evaluate", {
      expression: process.env.CDP_WHEN,
      returnByValue: true,
    });
    if (probe.result?.result?.value) break;
    if (Date.now() > deadline) throw new Error("CDP_WHEN timed out");
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

if (args[0] === "--screenshot") {
  const shot = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(args[1], Buffer.from(shot.result.data, "base64"));
  console.log(`saved ${args[1]}`);
} else {
  // `--file path.js` evaluates a script file (avoids shell quoting problems).
  const expression = args[0] === "--file" ? readFileSync(args[1], "utf8") : args[0];
  const result = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.result.exceptionDetails) {
    console.log("EXCEPTION", JSON.stringify(result.result.exceptionDetails.exception ?? result.result.exceptionDetails));
  } else {
    console.log(JSON.stringify(result.result.result.value, null, 2));
  }
}
socket.close();
