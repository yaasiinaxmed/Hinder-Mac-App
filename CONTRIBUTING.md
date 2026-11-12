# Contributing to Hinder

Thanks for helping make Hinder better! Bug reports, ideas and pull requests are all welcome.

## Reporting a bug or asking for a feature

Open an [issue](https://github.com/yaasiinaxmed/Hinder-Mac-App/issues) and include:

- your macOS version and Mac type (Apple silicon or Intel)
- the Hinder version (from the release you downloaded, or the commit you built)
- what you did, what you expected, and what happened instead
- a screenshot if it's a visual problem

Please don't include personal file names or paths you'd rather keep private.

## Setting up

You need macOS and [Rust](https://rustup.rs) 1.85 or newer.

```sh
git clone https://github.com/yaasiinaxmed/Hinder-Mac-App.git
cd Hinder-Mac-App
cargo run
```

Useful commands:

```sh
cargo test                     # run the tests
cargo fmt                      # format the code
cargo clippy                   # lint
./scripts/bundle.sh --native   # build dist/Hinder.app for your Mac
```

## Making a change

1. Fork the repo and create a branch: `git checkout -b fix/empty-downloads-crash`.
2. Make your change. Keep pull requests focused on one thing.
3. Run `cargo fmt` and `cargo test`.
4. Commit using a [Conventional Commit](https://www.conventionalcommits.org/) message (see below).
5. Push your branch and open a pull request against `main`. CI builds and tests it on macOS.

## Commit messages

The commit prefix decides the next version number, and the changelog is written from these messages, so please follow the format:

| Prefix | Use it for | Release |
| --- | --- | --- |
| `feat:` | a new feature | minor (0.1.0 → 0.2.0) |
| `fix:` | a bug fix | patch (0.1.0 → 0.1.1) |
| `feat!:` / `fix!:` | a breaking change | major once past 1.0 |
| `docs:` | README and docs only | none |
| `refactor:` `style:` `test:` `chore:` `ci:` `perf:` | everything else | none |

Examples:

```
feat: add music player to the toolbar
fix: don't crash when Downloads is empty
docs: explain how Clean Up picks files
```

## Ground rules for features

Hinder's promises to users are part of the design. Changes should keep them:

- **Local only.** No network calls, accounts, telemetry or uploads.
- **Never delete outright.** Anything removed goes to the Trash.
- **Don't download iCloud-only files** during scans.
- **Show your work.** Search and Clean Up should explain why a file matched.

## Releases

Maintainers don't tag releases by hand. A release-please bot keeps a "chore(main): release x.y.z" pull request open. Merging it creates the tag and the GitHub Release, and CI attaches the universal `.dmg` and `.zip`.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
