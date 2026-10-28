# Contributing

Thanks for helping. This page says how to build the project, how to check a
change, and what a pull request looks like.

## Requirements

- Rust with Cargo, rustfmt, and Clippy, at least the `rust-version` in
  [Cargo.toml](Cargo.toml). Install it with [rustup](https://rustup.rs).
- Node.js, which runs Prettier for documentation and configuration files.
- The [GitHub CLI](https://cli.github.com), authenticated, to run
  `git-koi collect` against a real account.

## Build

```sh
cargo build --release --locked
./target/release/git-koi --help
```

`--locked` keeps `Cargo.lock` as committed.

## Gates

Every pull request passes six gates: rustfmt, Clippy with warnings denied, the
tests, a release build, rustdoc with warnings denied, and Prettier over the
whole tree. The exact commands live in the CI workflow,
[.github/workflows/ci.yml](.github/workflows/ci.yml); run them locally before
pushing. To fix formatting rather than check it:

```sh
cargo fmt --all
npx prettier@3.9.10 --write .
```

## Pull Requests

- One topic per pull request. Independent changes travel separately.
- The commit subject is `area: lowercase imperative phrase stating the change`,
  where the area is the module or workflow, such as `render:` or `ci:`. The body
  says why in a few lines.
- Tests sit at public boundaries and fail under the bug they name. They do not
  pin incidental details.
- Documentation changes with the behavior it describes: doc comments and the
  README.
- Formatting changes touch only the lines the change touches.

## Issues and Security

Open bugs and feature requests through the issue forms. Report vulnerabilities
privately as [SECURITY.md](SECURITY.md) describes, never in a public issue.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).
