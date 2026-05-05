# State JSON

Betamax writes strict JSON for `State <path>.json` checkpoints and `Output <path>.json` final
terminal snapshots. The format is designed for snapshot testing first: plain text is easy to assert,
styled output is available when needed, and repeated per-cell default data is not emitted.

## Shape

```json
{
  "size": [80, 24],
  "total_rows": 31,
  "scrollback_rows": 7,
  "cursor": {
    "x": 2,
    "y": 10,
    "visible": true
  },
  "default_style": {
    "fg": "#dddddd",
    "bg": "#102040"
  },
  "styles": [{ "fg": "#5a56e0" }, { "fg": "#ffcc66", "bold": true }],
  "viewport_text": "> echo ok\nok\n> ",
  "scrollback_text": "cargo build\n...",
  "viewport": [[["> ", 0], "echo ok"], ["ok"], [["> ", 0]]],
  "scrollback": [["cargo build"], ["..."]]
}
```

`viewport_text` contains the active viewport after trailing blank rows are trimmed.
`scrollback_text` contains rows above the viewport, also with trailing blank rows trimmed. The
`viewport` and `scrollback` fields contain the same rows as compact styled spans.

## Span Rules

Rows are arrays of spans:

- A string span means text using `default_style`.
- A styled span is `[text, style_index]`.
- `style_index` points into `styles`.
- `styles` contains only non-default styles.
- Adjacent cells with the same style are merged into one span.
- Empty rows are represented as `[]`.
- Trailing empty rows are omitted because they are implied by `size`.

The `default_style` object records the theme-derived foreground and background used by plain string
spans. Non-default style objects omit fields that match `default_style`; for example a prompt
colored with the default background only records its foreground color.

## Style Fields

Style objects may include:

- `fg`
- `bg`
- `bold`
- `italic`
- `faint`
- `blink`
- `inverse`
- `invisible`
- `strikethrough`
- `overline`
- `underline`

Boolean fields are omitted when false. `underline` is omitted when it is `none`.

## Format Tradeoffs

Betamax keeps strict JSON as the canonical state format. It is deterministic, parseable with
standard libraries, and works well with snapshot tools such as `insta`.

YAML is easier to annotate by hand, but terminal text often contains characters that need YAML
quoting or escaping. That makes generated snapshots less predictable.

TOML is a poor fit for captured terminal state because nested mixed arrays of strings and styled
spans become awkward quickly. TOML is better for configuration than for structured terminal output.

JSONC is useful for human-authored fixtures, but it requires a non-standard parser. Betamax can add
a JSONC debug export later, but generated `.json` files stay strict so tests can consume them
without extra parser choices.
