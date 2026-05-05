# betamax

`betamax` is the installable CLI for Betamax, a Rust-first terminal capture tool in the spirit of
VHS.

```sh
cargo install betamax --locked
betamax run demo.tape
```

The CLI delegates tape parsing, terminal execution, rendering, and output writing to
`betamax-core`.

See the [repository README](https://github.com/joshka/betamax) for full usage and examples, and the
[Tape Reference](https://github.com/joshka/betamax/blob/main/docs/tape-reference.md) for command
behavior, settings, and defaults.
