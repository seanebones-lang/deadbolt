#!/usr/bin/env node
"use strict";

const crypto = require("crypto");
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

function down(urlPath, code = "store_unavailable") {
  if (urlPath.startsWith("/status")) return { code };
  return { decision: "deny", code };
}

function call(method, urlPath, body) {
  let payload;
  const headers = { "Content-Type": "application/json", Connection: "close" };
  if (process.env.DEADBOLT_ADMISSION_TOKEN !== undefined) headers["X-Deadbolt-Admission"] = process.env.DEADBOLT_ADMISSION_TOKEN;
  else if (process.env.DEADBOLT_TOKEN) headers["X-Deadbolt-Token"] = process.env.DEADBOLT_TOKEN;
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
          if (res.statusCode === 401 || res.statusCode === 403) {
            finish(down(urlPath, res.statusCode === 401 ? "unauthorized" : "forbidden"));
            return;
          }
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


function jsonSnapshot(value, depth = 0, budget = { left: 4096 }) {
  if (depth > 32 || budget.left-- <= 0) throw new TypeError("action limits");
  if (value === null || typeof value === "boolean") return value;
  if (typeof value === "string") {
    // UTF-8 encoding would otherwise silently replace lone surrogates.
    if (!value.isWellFormed()) throw new TypeError("invalid Unicode");
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || Math.abs(value) > Number.MAX_SAFE_INTEGER || Object.is(value, -0)) throw new TypeError("invalid action number");
    return value;
  }
  if (typeof value !== "object" || !value) throw new TypeError("JSON-native values required");
  const array = Array.isArray(value);
  if (!array && Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) throw new TypeError("plain JSON objects required");
  const result = array ? [] : Object.create(null);
  for (const key of Reflect.ownKeys(value)) {
    if (array && key === "length") continue;
    const property = Object.getOwnPropertyDescriptor(value, key);
    if (typeof key !== "string" || !property.enumerable || !("value" in property)) throw new TypeError("plain JSON properties required");
    jsonSnapshot(key);
    if (array && !/^(0|[1-9][0-9]*)$/.test(key)) throw new TypeError("plain JSON arrays required");
    result[key] = jsonSnapshot(property.value, depth + 1, budget);
  }
  if (array && Object.keys(result).length !== value.length) throw new TypeError("sparse arrays refused");
  return result;
}

function actionSnapshot(action) {
  const copy = jsonSnapshot(action);
  const fields = ["agent_id", "arguments", "dest", "expires_at", "nonce", "tool", "version"];
  if (!copy || Array.isArray(copy) || Object.keys(copy).sort().join() !== fields.join() || copy.version !== 1) throw new TypeError("invalid action envelope");
  for (const key of ["agent_id", "tool", "nonce", "dest"]) {
    if (key === "dest" && copy[key] === null) continue;
    if (typeof copy[key] !== "string" || !/^[A-Za-z0-9_.:/-]{1,128}$/.test(copy[key])) throw new TypeError("invalid action token");
  }
  if (!copy.arguments || Array.isArray(copy.arguments) || typeof copy.arguments !== "object" || !Number.isSafeInteger(copy.expires_at) || copy.expires_at <= 0 || Buffer.byteLength(JSON.stringify(copy)) > 32768) throw new TypeError("invalid action arguments/deadline");
  return copy;
}

function prepareAction(agentId, tool, argumentsValue, dest = null, ttlSecs = 300) {
  if (!Number.isInteger(ttlSecs) || ttlSecs < 1 || ttlSecs > 86400) throw new TypeError("invalid action lifetime");
  return actionSnapshot({ version: 1, agent_id: agentId, tool, dest, arguments: argumentsValue,
    nonce: crypto.randomBytes(32).toString("hex"), expires_at: Math.floor(Date.now() / 1000) + ttlSecs });
}

function admitAction(action) {
  let snapshot;
  try { snapshot = actionSnapshot(action); } catch { return Promise.resolve(down("/admit-action", "bad_request")); }
  return call("POST", "/admit-action", snapshot).then(result => {
    if (!result || (result.decision !== "allow" && result.decision !== "deny")) return down("/admit-action");
    return result;
  });
}

async function dispatchAction(action, body) {
  if (typeof body !== "function") throw new TypeError("action body must be callable");
  let snapshot;
  try { snapshot = actionSnapshot(action); } catch { return { executed: false, decision: down("/admit-action", "bad_request") }; }
  const decision = await admitAction(snapshot);
  if (decision.decision !== "allow") return { executed: false, decision };
  return { executed: true, decision, result: await body(snapshot.arguments) };
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

module.exports = { ensure, admit, dispatch, prepareAction, admitAction, dispatchAction, registerChild, status, policy, spend };

if (require.main === module) main().catch((err) => {
  console.error(String(err && err.message ? err.message : err));
  process.exit(1);
});
