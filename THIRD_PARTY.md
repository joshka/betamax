# Third-Party Notices

Betamax is distributed under either the MIT License or the Apache License, Version 2.0, at your
option.

## Ghostty Themes

Betamax includes copied Ghostty theme files under
`crates/betamax-core/resources/ghostty/themes`.

Upstream project:

- <https://github.com/ghostty-org/ghostty>

License:

- MIT

Upstream license notice:

```text
MIT License

Copyright (c) 2024 Mitchell Hashimoto, Ghostty contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

The copied theme files are distributed as data resources so Betamax can render by Ghostty theme name
without requiring a local Ghostty checkout.

Ghostty generates these files from iTerm2-Color-Schemes (<https://github.com/mbadolato/iTerm2-Color-Schemes>,
MIT License, Copyright (c) 2011 to Present Mark Badolato). Individual color schemes belong to their
authors, credited in that project.

## VHS Themes

Betamax includes VHS theme names and palettes under `crates/betamax-core/resources/vhs`, generated
by `scripts/import-vhs-themes.py` from VHS's `themes.json`. `aliases.json` maps VHS theme names to
Betamax theme names. `themes/` holds the VHS palettes that have no Ghostty counterpart; each file
names its original VHS theme and credits its author.

VHS took these themes from atomcorp/themes (<https://github.com/atomcorp/themes>, MIT License,
Copyright (c) 2019 to Thomas Maxwell Smith), which notes that the copyright and license for each
individual theme belongs to the author of that theme.

Upstream project:

- <https://github.com/charmbracelet/vhs>

License:

- MIT

Upstream license notice:

```text
MIT License

Copyright (c) 2022-2023 Charmbracelet, Inc

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## vhs-analyzer Tape Grammar

The docs site tape grammar at `site/src/grammars/tape.tmLanguage.json` is derived from the VHS tape
TextMate grammar in vhs-analyzer (`editors/code/syntaxes/tape.tmLanguage.json`, commit
`061c7c17d467eef358e59b754e653067f6fb72e0`) and extended for Betamax commands and settings.

Upstream project:

- <https://github.com/DrEden33773/vhs-analyzer>

License:

- MIT

Upstream license notice:

```text
MIT License

Copyright (c) 2026 DrEden33773

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## JetBrains Mono

The `betamax-fonts` crate embeds the JetBrains Mono 2.304 variable fonts under
`crates/betamax-fonts/fonts`, unmodified, as Betamax's default font. `betamax-core` depends on it
through the default `bundled-font` feature.

Upstream project:

- <https://github.com/JetBrains/JetBrainsMono>

License:

- SIL Open Font License 1.1, Copyright 2020 The JetBrains Mono Project Authors. The full text is in
  `crates/betamax-fonts/OFL.txt`. The OFL covers the font files only, not Betamax's code or the
  images it renders.

## tiny-skia

`betamax-core` renders with `tiny-skia` 0.12, and the `qwertty-term-sprite` rasterizer uses
`tiny-skia` 0.11. Both, with `tiny-skia-path`, are compiled into the `betamax` binary.

Upstream project:

- <https://github.com/linebender/tiny-skia>

License:

- BSD-3-Clause

Upstream license notice:

```text
Copyright (c) 2011 Google Inc. All rights reserved.
Copyright (c) 2020 Yevhenii Reizner All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

  * Redistributions of source code must retain the above copyright
    notice, this list of conditions and the following disclaimer.

  * Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in
    the documentation and/or other materials provided with the
    distribution.

  * Neither the name of the copyright holder nor the names of its
    contributors may be used to endorse or promote products derived
    from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```
