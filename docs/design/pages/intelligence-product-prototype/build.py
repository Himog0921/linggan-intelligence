"""Build the synthetic design reference; never connects to Linggan or platform APIs."""
import base64
import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import urllib.request

HERE = Path(__file__).resolve().parent
VERSION = "0.185.1"
INTEGRITY = "5aojFCXKwnjBRZvUnt3WFfEcvUJgkN5LlijRFN95hMy8WVkG4I0QNcJE+OuWvuJ0bOdStrbfXn0pkd6/QyiAlg=="
cache = Path(tempfile.gettempdir()) / f"linggan-design-three-{VERSION}.tgz"
if not cache.exists():
    with urllib.request.urlopen(f"https://registry.npmjs.org/three/-/three-{VERSION}.tgz", timeout=60) as response:
        raw = response.read()
else:
    raw = cache.read_bytes()
if base64.b64encode(hashlib.sha512(raw).digest()).decode() != INTEGRITY:
    raise SystemExit("Three.js package integrity mismatch")
cache.write_bytes(raw)
with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
    library = archive.extractfile("package/build/three.cjs").read().decode()
    license_text = archive.extractfile("package/LICENSE").read().decode()
assert "require(" not in library, "Expected self-contained Three.js CJS build"
css = (HERE / "style.css").read_text()
js = "\n".join((HERE / name).read_text() for name in ["data.js", "home3d.js", "corpus.js", "app.js"])
page = """<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="light"><title>Linggan Intelligence · 产品设计演示</title>
<style>""" + css + """</style></head><body>
<div class="prototype-strip">设计演示 · 全部为合成材料与预置分析 · 不连接实际系统 <span><button data-action="guide" class="quiet" style="font-size:12px;padding:2px 8px">走一遍情报循环</button> · 2026.09.06</span></div>
<div id="app"></div><dialog id="inspector" aria-labelledby="drawer-title"></dialog><div id="notice" role="status" aria-live="polite"></div>
<noscript>请启用 JavaScript 查看设计原型；产品职责与页面规格见配套设计文档。</noscript>
<script>/* Three.js """ + VERSION + "\n" + license_text + "*/\n(function(exports){\n" + library.replace("</script", "<\\/script") + "\n})(window.THREE={});</script>\n<script>" + js.replace("</script", "<\\/script") + "</script></body></html>\n"
target = HERE.parent / "intelligence-product-prototype.html"
target.write_text(page)
print(f"Built {target.name}: {target.stat().st_size:,} bytes; Three.js {VERSION}; sha256={hashlib.sha256(page.encode()).hexdigest()}")
