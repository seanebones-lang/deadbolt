#!/usr/bin/env node
"use strict";

const http = require("http");
const net = require("net");
const os = require("os");
const path = require("path");
const MAX_RESPONSE_BYTES = 1024 * 1024;
const REQUEST_TIMEOUT_MS = 5000;

function sockPath() {
  if (process.env.DEADBOLT_SOCK) return process.env.DEADBOLT_SOCK;
  if (process.platform === "win32") return "127.0.0.1:9782";
  return path.join(os.homedir(), ".deadbolt", "deadbolt.sock");
}

function tcpTarget(raw) {
  let text = raw;
  if (text.startsWith("http://")) text = text.slice("http://".length);
  else if (text.startsWith("https://")) throw new Error("deadbolt:bind_refused");
  if (text.startsWith("/") || text.startsWith(".") || text.includes("/")) return null;
  let host = null;
  let port = null;
  if (text.startsWith("[") && text.includes("]:")) {
    const end = text.indexOf("]");
    host = text.slice(1, end);
    port = text.slice(end + 2);
  } else if (text.includes(":")) {
    const cut = text.lastIndexOf(":");
    host = text.slice(0, cut);
    port = text.slice(cut + 1);
  }
  if (!host || !port || !/^\d+$/.test(port)) return null;
  if (host !== "127.0.0.1" && host !== "::1") throw new Error("deadbolt:bind_refused");
  const number = Number(port);
  if (!Number.isInteger(number) || number < 1 || number > 65535) throw new Error("deadbolt:bind_refused");
  return { host, port: number };
}

function down(urlPath) {
  if (urlPath.startsWith("/status")) return { code: "store_unavailable" };
  return { decision: "deny", code: "store_unavailable" };
}

function call(method, urlPath, body) {
  let payload;
  const headers = { "Content-Type": "application/json", Connection: "close" };
  if (process.env.DEADBOLT_TOKEN) headers["X-Deadbolt-Token"] = process.env.DEADBOLT_TOKEN;
  let target;
  try {
    payload = body == null ? null : JSON.stringify(body);
    target = tcpTarget(sockPath());
  } catch (err) {
    return Promise.resolve(down(urlPath));
  }
  if (payload) headers["Content-Length"] = Buffer.byteLength(payload);
  const options = target
    ? { host: target.host, port: target.port, method, path: urlPath, headers, timeout: REQUEST_TIMEOUT_MS }
    : {
        createConnection: () => {
          const sock = net.connect(sockPath());
          sock.setTimeout(REQUEST_TIMEOUT_MS, () => sock.destroy());
          return sock;
        },
        method,
        path: urlPath,
        headers,
        timeout: REQUEST_TIMEOUT_MS,
      };
  return new Promise((resolve) => {
    let settled = false;
    let req;
    let response;
    const finish = (obj, abort = false) => {
      if (settled) return;
      settled = true;
      clearTimeout(deadline);
      if (abort) {
        if (response) response.destroy();
        if (req) req.destroy();
      }
      resolve(obj);
    };
    // An idle socket timeout alone can be renewed indefinitely by partial data.
    const deadline = setTimeout(() => finish(down(urlPath), true), REQUEST_TIMEOUT_MS);
    try {
      req = http.request(options, (res) => {
        response = res;
        const chunks = [];
        let bytes = 0;
        res.on("error", () => finish(down(urlPath), true));
        res.on("aborted", () => finish(down(urlPath), true));
        res.on("data", (c) => {
          bytes += c.length;
          if (bytes > MAX_RESPONSE_BYTES) finish(down(urlPath), true);
          else if (!settled) chunks.push(c);
        });
        res.on("end", () => {
          const raw = Buffer.concat(chunks).toString("utf8");
          if (res.statusCode < 200 || res.statusCode >= 300 || !raw) {
            finish(down(urlPath));
            return;
          }
          try {
            finish(JSON.parse(raw));
          } catch {
            finish(down(urlPath));
          }
        });
      });
      req.on("timeout", () => finish(down(urlPath), true));
      req.on("error", () => finish(down(urlPath), true));
      if (payload) req.write(payload);
      req.end();
    } catch {
      // Invalid ports, headers and other synchronous HTTP errors also deny.
      finish(down(urlPath), true);
    }
  });
}

function ensure(agentId) {
  return call("POST", "/ensure", { agent_id: agentId });
}

function admit(agentId, tool, dest) {
  const body = { agent_id: agentId, tool };
  if (dest != null) body.dest = dest;
  return call("POST", "/admit", body).then((result) => {
    if (!result || (result.decision !== "allow" && result.decision !== "deny")) return down("/admit");
    return result;
  });
}

function policy(agentId, fields) {
  return call("POST", "/policy", { ...fields, agent_id: agentId });
}

async function dispatch(agentId, tool, body, dest) {
  if (typeof body !== "function") throw new TypeError("deadbolt dispatch requires a callable body");
  const decision = await admit(agentId, tool, dest);
  if (decision.decision !== "allow") return { executed: false, decision };
  // Tool errors propagate normally. Never retry an effect or mistake it for a
  // failed admission. Sync and async trusted callbacks are both supported.
  return { executed: true, decision, result: await body() };
}

function spend(agentId, usd) {
  return call("POST", "/spend", { agent_id: agentId, usd });
}

function registerChild(parent, child, swarmTaskId) {
  const body = { parent, child };
  if (swarmTaskId) body.swarm_task_id = swarmTaskId;
  return call("POST", "/register_child", body);
}

function status(agentId) {
  const urlPath = agentId ? "/status?agent=" + encodeURIComponent(agentId) : "/status";
  return call("GET", urlPath, null);
}

function arg(name) {
  const i = process.argv.indexOf(name);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

async function main() {
  const cmd = process.argv[2];
  let out;
  if (cmd === "ensure") out = await ensure(arg("--agent"));
  else if (cmd === "admit") out = await admit(arg("--agent"), arg("--tool"), arg("--dest"));
  else if (cmd === "register-child") {
    out = await registerChild(arg("--parent"), arg("--child"), arg("--swarm-task"));
  } else if (cmd === "status") out = await status(arg("--agent"));
  else {
    console.error("usage: admit|ensure|register-child|status");
    process.exit(1);
  }
  process.stdout.write(JSON.stringify(out) + "\n");
}

module.exports = { ensure, admit, dispatch, registerChild, status, policy, spend };

if (require.main === module) main().catch((err) => {
  console.error(String(err && err.message ? err.message : err));
  process.exit(1);
});
