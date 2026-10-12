"use strict";
/** Bind the shared static/React player. Return teardown for client navigation.
 * @param {ParentNode} root
 * @returns {() => void}
 */
function bindDeadBoltWalkthrough(root) {
 const video = root.querySelector("#walkthrough-video");
 const overview = root.querySelector("#overview-choice");
 const full = root.querySelector("#full-choice");
 const description = root.querySelector("#video-description");
 if (!video || !overview || !full || !description) return () => {};
 const controller = new AbortController();
 let current = "overview";
 let pendingSeek;
 function choose(kind, start) {
  video.pause();
  if (pendingSeek) video.removeEventListener("loadedmetadata", pendingSeek);
  pendingSeek = undefined;
  if (current !== kind) {
   current = kind;
   video.src = 'https://github.com/seanebones-lang/deadbolt/releases/download/walkthrough-2026-10-10/' + (kind === "full" ? "deadbolt-walkthrough-cedar-captioned.mp4" : "deadbolt-showcase-extract-cedar-captioned.mp4");
   video.load();
  }
  overview.setAttribute("aria-pressed", String(kind === "overview"));
  full.setAttribute("aria-pressed", String(kind === "full"));
  video.setAttribute("aria-label", "DeadBolt " + (kind === "full" ? "complete developer walkthrough" : "overview") + ", with narration and visible English captions");
  description.textContent = kind === "full" ? "Full walkthrough: 23 chapters covering installation, integration, policy, approval, revocation, evidence, recovery, and the actual Harness terminal and UI." : "Overview: the execution boundary, exact-action approval, actual Harness terminal and web UI, and how to start a bounded pilot.";
  if (start !== undefined) {
   const seek = () => { video.currentTime = start; video.focus(); pendingSeek = undefined; };
   if (video.readyState >= 1) seek();
   else { pendingSeek = seek; video.addEventListener("loadedmetadata", seek, { once: true, signal: controller.signal }); }
  }
 }
 overview.addEventListener("click", () => choose("overview"), { signal: controller.signal });
 full.addEventListener("click", () => choose("full"), { signal: controller.signal });
 root.querySelectorAll("[data-start]").forEach(button => button.addEventListener("click", () => choose("full", Number(button.dataset.start)), { signal: controller.signal }));
 return () => { controller.abort(); video.pause(); };
}
// STATIC_BOOTSTRAP: the company site imports the same binding without this line.
bindDeadBoltWalkthrough(document);
