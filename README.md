<p align="center">
  <img src="assets/icon.svg" width="128" alt="Hinder icon">
</p>

<h1 align="center">Hinder</h1>

<p align="center">
  A friendlier way to see, search and tidy the files on your Mac.<br>
  Open source, written in Rust, and everything stays on your Mac.
</p>

<p align="center">
  <a href="https://github.com/yaasiinaxmed/Hinder-Desktop-App/actions/workflows/ci.yml"><img src="https://github.com/yaasiinaxmed/Hinder-Desktop-App/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/yaasiinaxmed/Hinder-Desktop-App/releases/latest"><img src="https://img.shields.io/github/v/release/yaasiinaxmed/Hinder-Desktop-App?label=download" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/platform-macOS-lightgrey" alt="macOS">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT license"></a>
</p>

---

## Features

- **Library shelves.** Images, Videos, Music, Documents, Archives and Apps, each with big thumbnails.
- **Plain-English search.** Type `pdfs from last week over 5 mb` and Hinder shows how it understood you as filter chips you can change or remove.
- **Downloaded From.** Every download grouped by where it came from (Gmail, Slack, GitHub, any website), read from the metadata macOS already stores.
- **Duplicates.** Identical files found by content (SHA-256), with the space you'd get back.
- **Clean Up.** Suggestions for old installers, forgotten downloads, old screenshots, big old videos and unused apps.
- **Safe by default.** Nothing is ever deleted outright. Files go to the Trash so you can put them back.
- **Private.** No account, no network, no uploads.
- **Two looks.** Colourful or black-and-white. Switch with <kbd>⌃⌘T</kbd>.

## Download

Get the latest `.dmg` from the **[Releases page](https://github.com/yaasiinaxmed/Hinder-Desktop-App/releases/latest)**. It's a universal app for Apple silicon and Intel Macs.

### Opening the app

Hinder isn't notarized by Apple yet, so the first time you open it macOS will say it can't check the app. Either:

- right-click **Hinder.app**, choose **Open**, then **Open** again, or
- run `xattr -dr com.apple.quarantine /Applications/Hinder.app` in Terminal.

## Keyboard shortcuts

| Action | Shortcut |
| --- | --- |
| Search | <kbd>⌘F</kbd> |
| Home | <kbd>⇧⌘H</kbd> |
| Downloaded From | <kbd>⇧⌘J</kbd> |
| Duplicates | <kbd>⇧⌘D</kbd> |
| Clean Up | <kbd>⇧⌘K</kbd> |
| Grid / List | <kbd>⌘1</kbd> / <kbd>⌘2</kbd> |
| Bigger / smaller cards | <kbd>⌘+</kbd> / <kbd>⌘−</kbd> |
| Inspector | <kbd>⌘I</kbd> |
| Quick Look | <kbd>Space</kbd> |
| Open | <kbd>↩</kbd> or <kbd>⌘O</kbd> |
| Move to Trash | <kbd>⌘⌫</kbd> |
| Back | <kbd>⌘[</kbd> |
| Rescan | <kbd>⌘R</kbd> |
| Switch look | <kbd>⌃⌘T</kbd> |

## Build from source

You need macOS and [Rust](https://rustup.rs) 1.85 or newer.

```sh
git clone https://github.com/yaasiinaxmed/Hinder-Desktop-App.git
cd Hinder-Desktop-App

cargo run                        # run in development
cargo test                       # run the tests
./scripts/bundle.sh --native     # build dist/Hinder.app for this Mac
./scripts/bundle.sh              # universal .app, .zip and .dmg in dist/
```

## How it works

| Module | What it does |
| --- | --- |
| `scanner.rs` | Walks your home folders and `/Applications` in the background |
| `query.rs` | Turns plain-English searches into filters |
| `sources.rs` | Reads `kMDItemWhereFroms` to find where a download came from |
| `dupes.rs` | Size → partial hash → full SHA-256 duplicate detection |
| `cleanup.rs` | The rules behind each Clean Up card |
| `thumbs.rs` | Thumbnails via macOS Quick Look, cached on disk |
| `ui/` | The interface, built with [egui](https://github.com/emilk/egui) |

## Releases and versioning

Hinder uses [Semantic Versioning](https://semver.org) and releases are automated with [release-please](https://github.com/googleapis/release-please):

1. Commits to `main` use [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat: ...` adds a feature and bumps the **minor** version (0.1.0 → 0.2.0)
   - `fix: ...` fixes a bug and bumps the **patch** version (0.1.0 → 0.1.1)
   - `feat!: ...` or a `BREAKING CHANGE:` footer is a breaking change
   - `docs:`, `chore:`, `refactor:`, `style:`, `test:`, `ci:` don't trigger a release
2. A bot keeps a **release PR** open that bumps `Cargo.toml` and updates `CHANGELOG.md`.
3. Merging that PR tags the version, creates a GitHub Release, and attaches the built `.dmg` and `.zip`.

## Contributing

Issues and pull requests are welcome. Please run `cargo fmt` and `cargo test` before opening a PR, and use a Conventional Commit title.

## License

[MIT](LICENSE)
