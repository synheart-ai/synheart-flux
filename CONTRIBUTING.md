# Contributing to Synheart Flux

Thanks for taking the time to contribute.

## Ways to contribute

- Report bugs and request features via GitHub Issues
- Improve documentation and examples
- Submit pull requests for fixes and enhancements

## Development setup

Prereqs: Rust (stable).

```bash
cargo test
```

Recommended:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
```

## Pull requests

- Keep PRs focused and small when possible
- Add/adjust tests for behavior changes
- Ensure `cargo test` passes
- Ensure formatting and clippy are clean

## License

By contributing, you agree that your contributions will be licensed under the **Apache License 2.0** (see `LICENSE`).

