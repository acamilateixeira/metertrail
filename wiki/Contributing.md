# Contributing

Issues and pull requests are welcome. Small, focused changes are easiest to review.

## Before you open a pull request

```bash
cargo fmt
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
START_SERVER=1 ./scripts/demo.sh
```

CI runs the same commands and must pass.

## Guidelines

- **The contract comes first.** A change to status codes, fields, or limits updates the [API Reference](API-Reference), both READMEs, and the tests in the same pull request.
- **Every behavior has a test.** Concurrency changes need a test that runs requests in parallel.
- **Keep the demo honest.** If a guarantee changes, `scripts/demo.sh` and the Insomnia and Postman collections change with it.
- **Docs in two languages.** `README.md` (English) and `README.pt-BR.md` (Portuguese) stay in sync.
- **Commit messages** follow Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `ci:`).

## Editing this wiki

The wiki is generated from the `wiki/` folder of the repository and published by CI on every merge to `main`. Edit the files there through a pull request; edits made directly on the wiki page are overwritten.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](https://github.com/acamilateixeira/metertrail/blob/main/LICENSE).
