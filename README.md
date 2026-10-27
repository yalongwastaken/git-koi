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
├── examples/
├── src/
└── tests/
```

## Usage

### Requirements

- Stable Rust with Cargo, rustfmt, and Clippy. The Rust edition is declared in
  [Cargo.toml](Cargo.toml).
- A native linker for your target platform.
- Node.js and npm for documentation formatting with Prettier 3.9.10.
- The [GitHub CLI](https://cli.github.com), authenticated, for collection.

### Build and Run

Run from the repository root:

```sh
cargo fetch --locked
cargo build --release --offline --locked
./target/release/git-koi --help
```

### Collection

`git-koi collect LOGIN` gathers an account snapshot through `gh api` and writes
it as JSON to standard output. With `--output FILE`, it replaces the file only
after collection succeeds, so a failed run keeps the previous snapshot.

```sh
./target/release/git-koi collect octocat --output snapshot.json
```

The snapshot holds the account's numeric ID, its public repositories, the public
repositories GitHub counts it as contributing to, GitHub's color for each of
their primary languages, and its daily contribution counts from account creation
through the latest date GitHub reports. Only public data is read, so a GitHub
Actions workflow can collect with its built-in token:

```yaml
env:
  GH_TOKEN: ${{ github.token }}
```

The calendar is requested a year at a time and its dates are in UTC; the
observation date is the latest date GitHub reports. Before writing, the snapshot
is checked against the history rules below, so gaps, conflicting days, or a
calendar that starts after the account was created fail instead of producing a
partial snapshot. Success prints only the snapshot. Collection errors go to
standard error and exit 1; invalid usage exits 2. See
[the collection contract](src/collect.rs) and exercise it with
`cargo test --offline --locked --test collect`.

### Account Snapshots

The library exposes `history::AccountSnapshot` for reading and writing GitHub
repository observations through `serde_json`. Pass a snapshot to
`history::calculate_language_shares` to get language fractions by project count.
Collection and file access belong to the caller.

The snapshot contains repositories the account owns or GitHub recognizes it has
contributed to. Each distinct public, non-fork repository with a known primary
language contributes one vote. Repository order and identical duplicates do not
change the result; conflicting observations fail. Language names remain as
GitHub reports them.

Missing data fails instead of becoming an empty result. A confirmed empty
collection produces empty shares. The reader rejects unknown fields and
unsupported snapshot versions. See
[the snapshot types and calculation contract](src/history.rs) for the format and
[the synthetic account fixture](tests/fixtures/account.json) for an example.

Snapshots with contribution history also carry an observation date and daily
GitHub contribution counts. `history::summarize_contributions` returns the total
and chronological Monday-Sunday weeks, with active-day counts and observed-day
counts kept distinct. Explicitly inactive weeks remain in the result; partial
weeks retain their observed coverage. The observation date identifies the week
still in progress. Calculations use no system clock.

Dates may arrive in any order, and identical duplicates count once. A nonempty
history must contain every date from its first observation through its
observation date, including days with zero contributions. Gaps, conflicting
counts, and later dates fail instead of changing the apparent activity.

Collected snapshots also record GitHub's color for each language as `#rrggbb`,
or `null` when GitHub assigns none. The writer selects the snapshot version from
the available observations. Older snapshots remain readable for language shares;
contribution summaries report missing history rather than treating it as zero,
and koi development reports missing colors. See
[the dated account fixture](tests/fixtures/account-with-history.json).

Exercise the library through its public interface:

```sh
cargo test --offline --locked --test history
```

### Koi Development

`koi::derive_koi_development` turns a dated snapshot into reproducible
appearance inputs: an account identity seed, language shares with their GitHub
colors, contribution totals, and weekly streak profiles. The numeric account ID
supplies the seed, so changes to activity or language composition do not replace
the fish's identity.

The longest streak retains its weekly participation until a strictly longer
streak replaces it. The current streak remains independent, including when an
account returns below its record. An unfinished observation week cannot break a
streak. Once an inactive week completes, the latest streak and its end date
remain available for temporary markings to fade; later inactivity does not move
that date. Missing history fails rather than creating an undeveloped fish, and a
snapshot without language colors fails with a request to collect it again.

The model contains measurements, not pixels, growth thresholds, or fade
durations. See [the development contract](src/koi.rs) and exercise its public
interface with `cargo test --offline --locked --test koi`.

### Koi Rendering

`render::render_koi` draws a slim koi colored from its languages, with colored
outlines and dithered shading, into a transparent pixel frame. The body and fins
follow sampled curves, with shading that follows the body surface and fin rays
that follow each fin’s direction. The head widens from a round nose along a
gentle slant, and short pixel whiskers angle forward from the snout.
Contribution totals determine its size through `koi::calculate_growth_fraction`;
growth slows as the fish matures. Young koi have a rounder head, larger eyes, a
slimmer body, and short rounded fins, and grow into the adult shape by the
young-shape reference point. After the adult reference point, the body keeps
lengthening, broadens slightly, and its fins and tail lengthen. The maximum form
retains its anatomy at higher totals. Growth reference points live in
`src/koi.rs`, and anatomical proportions live in `src/render.rs`. Quiet days
alone do not change its base anatomy. The pose and framing account for the body
and fins.

Each language's GitHub color becomes the koi pigment nearest in hue: red (beni),
orange (orenji), yellow (ki), or blue (ai). Grays and languages without a color
become black (sumi). Languages on the same pigment add their shares, and equal
shares rank red, black, yellow, orange, then blue. Organic patches seeded by the
account give each pigment body area in proportion to its share, pulled a quarter
of the way toward the top pigment's classic variety: red plates, black
lightning, a netted blue back, or a netted solid yellow or orange coat. The
second pigment adds spots, or red, orange, or yellow sides under a blue back;
the third tints the fin roots. Young koi show their pigments pale until the
full-color reference point: red, orange, yellow, and blue come in first, and
black last.

`render::render_koi` takes one of six `coat::KoiBody` values from the caller:
white (Shiro, the default), black (Karasu), Asagi (blue-gray with a scale net, a
white head, and orange fins), tea (Cha), gold (Ogon), or platinum. Each body
redraws the pigments in its own palette, and a pigment that would match the
ground swaps so it stays visible: black becomes white on black, blue becomes
white on Asagi, yellow becomes platinum on gold and gold on platinum, and tea
draws every pigment in earthy tones. Black and Asagi also brighten yellow, and
black turns blue sky blue. Gold and platinum catch a metallic sheen on the bare
body, never on the color patches. A koi without counted languages shows its
plain body. Streak markings are not painted yet. Pigment rules, bodies, and
palettes live in `src/coat.rs`.

Every growth level shares a square native canvas. `output::encode_png` exports
it with exact 2x pixel scaling, preserving crisp edges and transparency.
`output::encode_svg` returns a self-contained vector document of the same pixels
at the same display size. The SVG uses native pixel coordinates and groups
neighboring equal pixels into row rectangles. Canvas dimensions live in
`src/render.rs`; static export dimensions derive from them in `src/output.rs`.

Rendering and encoding are deterministic and leave file access and publication
to the caller.

Exercise the rendering and encoding boundary with
`cargo test --offline --locked --test render`.

### Koi Library

The generated library shows one-, two-, three-, and four-language mixes on the
default white body, then each body with one language per pigment, across growth
samples. Each sample is a transparent PNG/SVG pair, and the mixes and bodies
each get light and dark comparison sheets. The mixes use real GitHub language
colors on a synthetic account, so the samples show the color rule, the variety
layouts, the body palettes, and the pale young stage.

Generate the library from the repository root, then open `library/README.md`.
Git ignores the generated files.

```sh
cargo run --offline --locked --example generate-library -- library
```

The example owns the language mixes and growth totals. It uses a synthetic
account fixture and the production renderer and encoders. Generation replaces
matching files; it does not remove other files. Rerun after an interrupted
generation to repair a partial set.

For each pull request and push to `main`, the library workflow generates a fresh
library and attaches it to the run as the `koi-library` artifact. Download it
from the run's summary page to review rendering changes.

### Manual Checks

```sh
cargo fmt --all -- --check
cargo clippy --offline --locked --all-targets -- -D warnings
cargo test --offline --locked
cargo build --release --offline --locked
npx prettier@3.9.10 --check README.md .prettierrc.json 'tests/fixtures/*.json' .github/workflows/library.yml
```

`--locked` prevents changes to `Cargo.lock`; `--offline` prevents Cargo from
accessing the network. Fetch dependencies before running the offline checks. The
Prettier command may download the formatter if it is not cached.

## Author

**Anthony Yalong**

- Email: yalong.anthony123@gmail.com
- GitHub: [@yalongwastaken](https://github.com/yalongwastaken)

## License

MIT. See [LICENSE](LICENSE).
