# Git koi

Git koi is a Rust project for generating distinctive animated pixel-art koi from
GitHub contribution history and repository language shares.

> **Status: in progress.**

## Repository Structure

```text
.
├── .gitignore
├── .prettierrc.json
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
└── src/
```

## Usage

### Requirements

- Stable Rust with Cargo, rustfmt, and Clippy. The Rust edition is declared in
  [Cargo.toml](Cargo.toml).
- A native linker for your target platform.
- Node.js and npm for documentation formatting with Prettier 3.9.10.

### Build and Run

Run from the repository root:

```sh
cargo build --release --offline --locked
./target/release/git-koi
```

The executable is a scaffold: it exits successfully without output. It does not
generate koi or provide command-line options yet.

### Manual Checks

```sh
cargo fmt --all -- --check
cargo clippy --offline --locked --all-targets -- -D warnings
cargo test --offline --locked
cargo build --release --offline --locked
npx prettier@3.9.10 --check README.md .prettierrc.json
```

`--locked` prevents changes to `Cargo.lock`; `--offline` prevents Cargo from
accessing the network. The scaffold has no tests, so `cargo test` reports zero
tests. The Prettier command may download the formatter if it is not cached.

## Author

**Anthony Yalong**

- Email: yalong.anthony123@gmail.com
- GitHub: [@yalongwastaken](https://github.com/yalongwastaken)

## License

MIT. See [LICENSE](LICENSE).
