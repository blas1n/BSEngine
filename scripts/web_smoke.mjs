// Runs a web build (`scripts/build_web.sh`) in a real headless browser and
// fails unless the game draws.
//
// usage: node scripts/web_smoke.mjs <web-build-dir> [--frames N] [--timeout S]
//   BROWSER=<path>    Chrome, Chromium or Edge; found on PATH / the usual
//                     install locations otherwise.
//   WEBGPU_ADAPTER=swiftshader
//                     WebGPU on the CPU, for a machine with no GPU (CI).
//
// Passes when the page's `#bsengine-status` reports `frames >= N` with at
// least one draw call, and nothing was logged as an error on the way:
// no uncaught exception, no WebGPU validation error, no failed fetch, no
// engine `ERROR` line. Frames alone would not do -- a page whose renderer
// never came up still counts frames -- and neither would the status alone:
// a shader the browser rejects leaves draw calls counted and the canvas
// black, with only a console warning ("Error while parsing WGSL: ...") to say
// so.
//
// Node 22+ (global fetch and WebSocket); no dependencies.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { extname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : fallback;
};
const wantFrames = Number(opt('--frames', '10'));
const timeoutS = Number(opt('--timeout', '120'));
const root = resolve(args[0] ?? '');
if (!args[0] || !existsSync(join(root, 'index.html'))) {
  console.error('usage: node scripts/web_smoke.mjs <web-build-dir> [--frames N] [--timeout S]');
  process.exit(2);
}

// -- serve the build ---------------------------------------------------------
const types = {
  '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm',
  '.pak': 'application/octet-stream',
};
const server = createServer((req, res) => {
  const path = join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname));
  if (!path.startsWith(root) || !existsSync(path) || !statSync(path).isFile()) {
    // The browser's own report of a 404 does not say which file.
    console.log(`  404 ${req.url}`);
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' });
  res.end(readFileSync(path));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const pageUrl = `http://127.0.0.1:${server.address().port}/index.html`;

// -- start the browser -------------------------------------------------------
const candidates = [
  process.env.BROWSER,
  'google-chrome', 'google-chrome-stable', 'chromium', 'chromium-browser',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
].filter(Boolean);
const profile = mkdtempSync(join(tmpdir(), 'bse-web-smoke-'));
const flags = [
  '--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--enable-unsafe-webgpu',
  // Linux's WebGPU is Vulkan-backed and off by default.
  '--enable-features=Vulkan', '--window-size=1280,720',
];
if (process.env.WEBGPU_ADAPTER) {
  flags.push(`--use-webgpu-adapter=${process.env.WEBGPU_ADAPTER}`, '--enable-unsafe-swiftshader');
}
let browser;
for (const exe of candidates) {
  if (exe.includes('/') && !existsSync(exe)) continue;
  try {
    browser = spawn(exe, [...flags, 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
    await new Promise((ok, fail) => { browser.once('spawn', ok); browser.once('error', fail); });
    break;
  } catch { browser = undefined; }
}
if (!browser) {
  console.error(`no browser found (tried ${candidates.join(', ')}); set BROWSER`);
  process.exit(2);
}

const failures = [];
let finished = false;
const finish = (code) => {
  if (finished) return;
  finished = true;
  browser.kill();
  server.close();
  setTimeout(() => {
    try { rmSync(profile, { recursive: true, force: true }); } catch {}
    process.exit(code);
  }, 500);
};
setTimeout(() => {
  console.error(`FAIL: no ${wantFrames} drawn frames within ${timeoutS}s`);
  finish(1);
}, timeoutS * 1000);

// The DevTools endpoint is announced on stderr once the browser listens.
const wsUrl = await new Promise((ok) => {
  let text = '';
  browser.stderr.on('data', (d) => {
    text += d;
    const m = text.match(/DevTools listening on (ws:\/\/\S+)/);
    if (m) ok(m[1]);
  });
});
const port = new URL(wsUrl).port;
const page = (await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json());

// -- drive the page ----------------------------------------------------------
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener('open', r));
let nextId = 1;
const pending = new Map();
const send = (method, params = {}) => new Promise((r) => {
  const id = nextId++;
  pending.set(id, r);
  ws.send(JSON.stringify({ id, method, params }));
});
ws.addEventListener('message', (e) => {
  const msg = JSON.parse(e.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); return; }
  if (msg.method === 'Runtime.consoleAPICalled') {
    // tracing-wasm styles its lines: "%cLEVEL%c file:line%c message", css...
    const text = msg.params.args.map((a) => a.value ?? a.description ?? '').join(' ');
    const level = text.match(/^%c(\w+)%c/)?.[1];
    if (msg.params.type === 'error' || level === 'ERROR') failures.push(`console: ${text}`);
    if (level === 'ERROR' || level === 'WARN' || (level === 'INFO' && process.env.VERBOSE)) {
      console.log(`  ${level} ${text.replace(/%c/g, '').replace(/ color:.*$/, '')}`);
    }
  } else if (msg.method === 'Log.entryAdded') {
    // Chrome reports WebGPU's own errors -- a shader it will not compile, a
    // pipeline that is therefore invalid -- as *warnings*, while the frame
    // goes on counting its draw calls on the CPU. Those are failures; the
    // other warnings a page can draw (the AudioContext waiting for a click)
    // are not.
    const { level, text } = msg.params.entry;
    if (level === 'error' || (level === 'warning' && /WGSL|WebGPU|GPU|Invalid|error/i.test(text))) {
      failures.push(`browser ${level}: ${text}`);
    }
  } else if (msg.method === 'Runtime.exceptionThrown') {
    const d = msg.params.exceptionDetails;
    failures.push(`exception: ${d.exception?.description ?? d.text}`);
  }
});
await send('Runtime.enable');
await send('Log.enable');
await send('Page.navigate', { url: pageUrl });
console.log(`web smoke: ${pageUrl} (want ${wantFrames} frames)`);

let last = '';
for (;;) {
  await new Promise((r) => setTimeout(r, 500));
  if (failures.length) {
    console.error(`FAIL:\n  ${failures.join('\n  ').slice(0, 4000)}`);
    finish(1);
    break;
  }
  const res = await send('Runtime.evaluate', {
    expression: "document.getElementById('bsengine-status')?.textContent ?? ''",
    returnByValue: true,
  });
  const status = res.result?.result?.value ?? '';
  if (status !== last && !/^frames=/.test(status)) console.log(`  status: ${status}`);
  last = status;
  if (status.startsWith('error')) {
    console.error(`FAIL: ${status}`);
    finish(1);
    break;
  }
  const m = status.match(/frames=(\d+) draw_calls=(\d+)/);
  if (m && Number(m[1]) >= wantFrames) {
    if (Number(m[2]) === 0) {
      console.error(`FAIL: ${status} -- frames ran, nothing was drawn`);
      finish(1);
    } else {
      console.log(`PASS: ${status}`);
      finish(0);
    }
    break;
  }
}
