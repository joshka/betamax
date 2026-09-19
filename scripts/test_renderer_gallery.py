"""Validate report escaping and partial/absent checkpoint handling without a renderer."""

import base64
from html.parser import HTMLParser
from pathlib import Path
import runpy
import tempfile
import unittest

build_gallery = runpy.run_path(str(Path(__file__).with_name("renderer-gallery.py")))["build_gallery"]


class Images(HTMLParser):
    def __init__(self):
        super().__init__()
        self.sources = []
        self.tags = []

    def handle_starttag(self, tag, attrs):
        self.tags.append(tag)
        if tag == "img":
            self.sources.append(dict(attrs)["src"])


class GalleryTests(unittest.TestCase):
    def test_absent_checkpoints_remove_stale_gallery(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "gallery.html"
            output.write_text("stale")
            self.assertEqual(build_gallery(root / "absent", output, "Linux", "failure", "abc"), 0)
            self.assertFalse(output.exists())

    def test_partial_failed_run_embeds_bytes_and_escapes_untrusted_text(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            # PNG signature is enough here: the report must preserve, not re-encode, its input.
            pixels = b"\x89PNG\r\n\x1a\n"
            (root / "layout.png").write_bytes(pixels)
            (root / 'label<img src=x>.json').write_text('{"text": "</pre><script>alert(1)</script>"}')
            (root / "unicode.json").write_text("invalid <b>JSON")
            output = root / "gallery.html"
            count = build_gallery(root, output, '<img src="remote">', "failure", "<revision>")
            document = output.read_text()
            parsed = Images()
            parsed.feed(document)
            self.assertEqual(count, 3)
            self.assertEqual(len(parsed.sources), 1)
            self.assertEqual(base64.b64decode(parsed.sources[0].split(",", 1)[1]), pixels)
            self.assertNotIn("script", parsed.tags)
            self.assertNotIn("b", parsed.tags)
            self.assertIn('&lt;img src=&quot;remote&quot;&gt;', document)
            self.assertIn("label&lt;img src=x&gt;", document)
            self.assertIn("&lt;/pre&gt;&lt;script&gt;", document)
            self.assertIn("Invalid JSON checkpoint:", document)
            self.assertIn("JSON checkpoint unavailable.", document)
            self.assertIn("PNG checkpoint unavailable.", document)
            self.assertIn("Missing checkpoints:", document)
            self.assertIn("<strong>failure</strong>", document)
            self.assertIn("Wide-glyph acceptance checks run in normal CI", document)
            self.assertIn("wide-glyph, wide-styled, wide-cursor", document)


if __name__ == "__main__":
    unittest.main()
