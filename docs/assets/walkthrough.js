"use strict";
(() => {
 const video = document.getElementById("walkthrough-video");
 const overview = document.getElementById("overview-choice");
 const full = document.getElementById("full-choice");
 const description = document.getElementById("video-description");
 let current = "overview";
 function choose(kind, start) {
  video.pause();
  if (current !== kind) {
   current = kind;
   video.src = 'https://github.com/seanebones-lang/deadbolt/releases/download/walkthrough-2026-10-10/' + (kind === "full" ? "deadbolt-walkthrough-cedar-captioned.mp4" : "deadbolt-showcase-extract-cedar-captioned.mp4");
   video.load();
  }
  overview.setAttribute("aria-pressed", String(kind === "overview"));
  full.setAttribute("aria-pressed", String(kind === "full"));
  video.setAttribute("aria-label", "DeadBolt " + (kind === "full" ? "complete developer walkthrough" : "overview") + ", with narration and visible English captions");
  description.textContent = kind === "full" ? "Full walkthrough: 23 chapters covering installation, integration, policy, approval, revocation, evidence, recovery, and the actual Harness terminal and UI." : "Overview: the execution boundary, exact-action approval preview, actual Harness terminal and web UI, and how to start a bounded pilot.";
  if (start !== undefined) {
   const seek = () => { video.currentTime = start; video.focus(); };
   if (video.readyState >= 1) seek();
   else video.addEventListener("loadedmetadata", seek, { once: true });
  }
 }
 overview.addEventListener("click", () => choose("overview"));
 full.addEventListener("click", () => choose("full"));
 document.querySelectorAll("[data-start]").forEach(button => button.addEventListener("click", () => choose("full", Number(button.dataset.start))));
})();
