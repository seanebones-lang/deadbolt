"""Render public editor guides. Build dependency: markdown-it-py==4.2.0."""
import html
from pathlib import Path
from urllib.parse import urljoin
from markdown_it import MarkdownIt

DOCS = Path(__file__).resolve().parents[1] / "docs"
PAGES = {"EDITORS.md": "editors.html", "EDITOR-TUTORIAL.md": "editor-tutorial.html", "EDITORS-V2.md": "editors-v2.html"}


def main():
    renderer = MarkdownIt("commonmark", {"html": False}).enable("table")
    for source, output in PAGES.items():
        markdown = (DOCS / source).read_text(encoding="utf-8")
        tokens = renderer.parse(markdown)
        for token in tokens:
            for child in token.children or []:
                if child.type != "link_open":
                    continue
                href = child.attrGet("href")
                if not href or ":" in href or href.startswith("#"):
                    continue
                file, separator, fragment = href.partition("#")
                if file in PAGES:
                    child.attrSet("href", PAGES[file] + (separator + fragment if fragment else ""))
                elif not file.startswith("assets/"):
                    child.attrSet("href", urljoin("https://github.com/seanebones-lang/deadbolt/blob/main/docs/", href))
        title = markdown.splitlines()[0].lstrip("# ")
        helper = "editor_setup_v2.py" if source == "EDITORS-V2.md" else "editor_setup.py"
        helper_label = "Download stable 2.0.0 helper" if source == "EDITORS-V2.md" else "Download historical 1.x helper"
        body = renderer.renderer.render(tokens, renderer.options, {})
        body = body.replace("<table>", '<div class="guide-table-wrap"><table>').replace("</table>", "</table></div>")
        page = '''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>''' + html.escape(title) + ''' · DeadBolt</title>
<meta name="description" content="Install DeadBolt in VS Code and Cursor and verify denial, approval and revocation with disposable local MCP tools.">
<link rel="icon" href="assets/favicon.svg"><link rel="stylesheet" href="assets/site.css">
<style>.guide{max-width:860px;padding-block:54px 80px}.guide h1{font-size:clamp(2.1rem,5vw,3.8rem);line-height:1.1;letter-spacing:-.045em}.guide h2{font-size:1.65rem;margin-top:2.8rem;letter-spacing:-.02em}.guide p,.guide li{font-size:1.06rem;line-height:1.8}.guide a{overflow-wrap:anywhere;text-underline-offset:3px}.guide pre{padding:22px;background:#151719;color:#f6f2ec;overflow-x:auto;line-height:1.7;border-radius:4px}.guide pre code{font-size:.9rem;white-space:pre}.guide code{font-family:var(--mono);font-size:.88em;overflow-wrap:anywhere}.guide-table-wrap{overflow-x:auto}.guide table{border-collapse:collapse;width:100%}.guide th,.guide td{border:1px solid var(--line);padding:14px;text-align:left;line-height:1.5}.guide-nav{display:flex;flex-wrap:wrap;gap:18px;font-weight:700;margin-bottom:28px}.download-callout{padding:22px;border:1px solid var(--line);margin-bottom:30px;line-height:1.7}.guide footer{border-top:1px solid var(--line);padding-top:25px;margin-top:50px}@media(max-width:760px){.guide{width:calc(100% - 36px)}.guide pre{padding:14px}.topbar nav{gap:1rem}.topbar nav a{font-size:.75rem}}</style>
</head><body><a class="skip" href="#content">Skip to content</a>
<header class="topbar"><a class="wordmark" href="index.html">DEADBOLT</a><nav aria-label="Main navigation"><a href="index.html#install">Downloads</a><a href="index.html#watch">Watch</a><a href="https://github.com/seanebones-lang/deadbolt">Source</a></nav></header>
<main class="guide wrap" id="content"><nav class="guide-nav" aria-label="Editor guides"><a href="editors.html">Editor installation</a><a href="editor-tutorial.html">Practical tutorial</a><a href="editors-v2.html">Stable 2.0.0 editor guide</a><a href="assets/''' + helper + '''" download="''' + helper + '''">''' + helper_label + '''</a></nav>
<div class="download-callout"><strong>Start locally.</strong> Python 3.9+, Node.js 20+ and a checksum-verified bundle. No model account is needed for the disposable protocol test. Read-only setup provides no active protection.</div>
''' + body + '''<footer>Free and open source · Apache-2.0 · <a href="https://www.nexteleven.dev/deadbolt">Built by NextEleven</a></footer></main></body></html>
'''
        (DOCS / output).write_text(page, encoding="utf-8")


if __name__ == "__main__":
    main()
