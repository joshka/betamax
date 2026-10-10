"""Check the worked example's hydrated source and built media against recorded hashes."""

import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("root", type=Path, help="site/public or the built site/dist directory")
args = parser.parse_args()
assets = args.root / "assets/sticky-headers"
manifest = json.loads((assets / "media.json").read_text())
for name, expected in manifest.items():
    data = (assets / name).read_bytes()
    magic = b"GIF89a" if name.endswith(".gif") else b"\x89PNG\r\n\x1a\n"
    if not data.startswith(magic):
        raise SystemExit(f"{name}: expected hydrated image bytes")
    if hashlib.sha256(data).hexdigest() != expected:
        raise SystemExit(f"{name}: checksum mismatch")
print(f"Verified {len(manifest)} worked-example images in {args.root}")

page = args.root / "testing/visual-development/index.html"
if page.exists():
    html = page.read_text()
    for name in manifest:
        if f'src="/betamax/assets/sticky-headers/{name}"' not in html:
            raise SystemExit(f"{name}: built page does not reference its subpath asset")
    print("Verified built page image URLs include the /betamax/ base")
