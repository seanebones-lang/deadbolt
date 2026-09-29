#!/usr/bin/env node
"use strict";

const http = require("http");
const net = require("net");
const os = require("os");
const path = require("path");

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
  return { host, port: Number(port) };
}

function down(urlPath) {
  if (urlPath.startsWith("/status")) return { code: "store_unavailable" };
  return { decision: "deny", code: "store_unavailable" };
}

function call(method, urlPath, body) {
  const payload = body == null ? null : JSON.stringify(body);
  const headers = { "Content-Type": "application/json", Connection: "close" };
  if (process.env.DEADBOLT_TOKEN) headers["X-Deadbolt-Token"] = process.env.DEADBOLT_TOKEN;
  if (payload) headers["Content-Length"] = Buffer.byteLength(payload);
  let target;
  try {
    target = tcpTarget(sockPath());
  } catch (err) {
    return Promise.resolve(down(urlPath));
  }
  const options = target
    ? { host: target.host, port: target.port, method, path: urlPath, headers, timeout: 5000 }
    : {
        createConnection: () => {
          const sock = net.connect(sockPath());
          sock.setTimeout(5000, () => sock.destroy());
          return sock;
        },
        method,
        path: urlPath,
        headers,
        timeout: 5000,
      };
  return new Promise((resolve) => {
    let settled = false;
    const finish = (obj) => {
      if (settled) return;
      settled = true;
      resolve(obj);
    };
    const req = http.request(options, (res) => {
      const chunks = [];
      res.on("error", () => finish(down(urlPath)));
      res.on("aborted", () => finish(down(urlPath)));
      res.on("data", (c) => chunks.push(c));
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
    req.on("timeout", () => {
      req.destroy();
      finish(down(urlPath));
    });
    req.on("error", () => finish(down(urlPath)));
    if (payload) req.write(payload);
    req.end();
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

module.exports = { ensure, admit, registerChild, status, policy, spend };

if (require.main === module) main().catch((err) => {
  console.error(String(err && err.message ? err.message : err));
  process.exit(1);
});
