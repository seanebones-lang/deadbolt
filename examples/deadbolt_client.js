#!/usr/bin/env node
"use strict";

const http = require("http");
const net = require("net");
const os = require("os");
const path = require("path");

function sockPath() {
  if (process.env.DEADBOLT_SOCK) return process.env.DEADBOLT_SOCK;
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

function call(method, urlPath, body) {
  const payload = body == null ? null : JSON.stringify(body);
  const headers = { "Content-Type": "application/json", Connection: "close" };
  if (process.env.DEADBOLT_TOKEN) headers["X-Deadbolt-Token"] = process.env.DEADBOLT_TOKEN;
  if (payload) headers["Content-Length"] = Buffer.byteLength(payload);
  const target = tcpTarget(sockPath());
  const options = target
    ? { host: target.host, port: target.port, method, path: urlPath, headers }
    : {
        createConnection: () => net.connect(sockPath()),
        method,
        path: urlPath,
        headers,
      };
  return new Promise((resolve, reject) => {
    const req = http.request(options, (res) => {
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => {
          const raw = Buffer.concat(chunks).toString("utf8");
          if (!raw) {
            reject(new Error("deadbolt:empty"));
            return;
          }
          resolve(JSON.parse(raw));
        });
      }
    );
    req.on("error", reject);
    if (payload) req.write(payload);
    req.end();
  });
}

function ensure(agentId) {
  return call("POST", "/ensure", { agent_id: agentId });
}

function admit(agentId, tool) {
  return call("POST", "/admit", { agent_id: agentId, tool });
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
  else if (cmd === "admit") out = await admit(arg("--agent"), arg("--tool"));
  else if (cmd === "register-child") {
    out = await registerChild(arg("--parent"), arg("--child"), arg("--swarm-task"));
  } else if (cmd === "status") out = await status(arg("--agent"));
  else {
    console.error("usage: admit|ensure|register-child|status");
    process.exit(1);
  }
  process.stdout.write(JSON.stringify(out) + "\n");
}

main().catch((err) => {
  console.error(String(err && err.message ? err.message : err));
  process.exit(1);
});
