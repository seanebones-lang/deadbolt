const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
class Element extends EventTarget {
  constructor(start) { super(); this.dataset = { start }; this.attributes = {}; this.readyState = 0; this.currentTime = 0; }
  setAttribute(k, v) { this.attributes[k] = v; }
  pause() {} load() { this.readyState = 0; } focus() { this.focused = true; }
  click() { this.dispatchEvent(new Event('click')); }
}
const video = new Element(), overview = new Element(), full = new Element(), description = new Element();
const chapters = [new Element('53.882'), new Element('995.316')];
const nodes = { '#walkthrough-video': video, '#overview-choice': overview, '#full-choice': full, '#video-description': description };
const root = { querySelector: s => nodes[s], querySelectorAll: () => chapters };
const context = vm.createContext({ document: root, AbortController });
vm.runInContext(readFileSync('docs/assets/walkthrough.js', 'utf8').split('// STATIC_BOOTSTRAP:')[0], context);
const teardown = context.bindDeadBoltWalkthrough(root);
chapters[0].click(); chapters[1].click(); video.readyState = 1; video.dispatchEvent(new Event('loadedmetadata'));
assert.equal(video.currentTime, 995.316, 'only the newest pending chapter may seek');
assert.equal(full.attributes['aria-pressed'], 'true');
video.readyState = 0; chapters[0].click(); overview.click(); video.dispatchEvent(new Event('loadedmetadata'));
assert.equal(video.currentTime, 995.316, 'switching to overview cancels pending full-video seek');
assert.match(video.src, /showcase-extract/);
assert.equal(overview.attributes['aria-pressed'], 'true');
teardown(); const oldSource = video.src; full.click(); assert.equal(video.src, oldSource, 'navigation removes listeners');
const teardownAgain = context.bindDeadBoltWalkthrough(root); full.click(); assert.match(video.src, /walkthrough-cedar/); teardownAgain();
assert.doesNotThrow(() => context.bindDeadBoltWalkthrough({ querySelector: () => null })());
console.log('Landing player: latest chapter, source-switch cancellation, controls, teardown, rebind and missing markup passed.');
