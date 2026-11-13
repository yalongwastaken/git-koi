use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use chrono::{DateTime, NaiveDate};
use git_koi::behavior::{Behavior, select_behavior};
use git_koi::coat::KoiBody;
use git_koi::collect::{GhCli, collect_account_snapshot};
use git_koi::history::AccountSnapshot;
use git_koi::koi::derive_koi_development;
use git_koi::output::{
    MAX_EXPORT_SCALE, STATIC_EXPORT_SCALE, encode_gif_scaled, encode_png_scaled,
    encode_sized_preview_html,
};
use git_koi::pond::PondSize;
use git_koi::render::{KoiFrame, SwimLoop, render_koi_in};

const USAGE: &str = "Usage: git-koi collect LOGIN [--output FILE]
       git-koi render SNAPSHOT --output-dir DIR [--body BODY] [--size WxH] [--scale N]
       git-koi --help | --version
Collect LOGIN's public GitHub snapshot as JSON through `gh api`: public owned and
contributed repositories, and daily contributions from the account's creation
through yesterday (UTC), the last complete day.
Writes to standard output, or replaces FILE only after collection succeeds.
Requires an authenticated GitHub CLI; in GitHub Actions, set GH_TOKEN to github.token.
Render a collected SNAPSHOT into DIR: koi.gif swims the behavior chosen for its
observation date, koi.png is the still koi, and preview.html shows both on light and
dark backgrounds. BODY is white (the default), black, asagi, tea, gold, or platinum.
The pond is 256x256 native pixels unless --size gives another, each side from 256
through 4096; in a pond that is not square the koi swims a new track each day, and a
longer track makes a longer loop. Each native pixel becomes N pixels on each side, 2
unless --scale gives 1 through 4. A GIF's weight grows with its area and length, and
phones may not play a heavy one: for a wide pond, try --size 768x256 --scale 1.
The options may come in any order. The files are replaced only after all of them render.
Errors go to stderr and exit 1; invalid usage exits 2.
Examples: git-koi collect octocat --output snapshot.json
          git-koi render snapshot.json --output-dir koi --body asagi
          git-koi render snapshot.json --output-dir koi --size 768x256 --scale 1";

/// Each body's name on the command line.
const BODY_NAMES: [(&str, KoiBody); 6] = [
    ("white", KoiBody::White),
    ("black", KoiBody::Black),
    ("asagi", KoiBody::Asagi),
    ("tea", KoiBody::Tea),
    ("gold", KoiBody::Gold),
    ("platinum", KoiBody::Platinum),
];

fn main() -> ExitCode {
    let program_arguments: Vec<OsString> = env::args_os().skip(1).collect();
    let argument_texts: Vec<Option<&str>> = program_arguments
        .iter()
        .map(|argument| argument.to_str())
        .collect();
    match argument_texts.as_slice() {
        [Some("--help")] | [Some("collect" | "render"), Some("--help")] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        [Some("--version")] => {
            println!("git-koi {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [Some("collect"), Some(login)] if !login.starts_with('-') => run_collect(login, None),
        [Some("collect"), Some(login), Some("--output"), _] if !login.starts_with('-') => {
            run_collect(login, Some(Path::new(&program_arguments[3])))
        }
        [Some("render"), Some(snapshot_path), ..] if !snapshot_path.starts_with('-') => {
            match parse_render_options(&program_arguments[2..]) {
                Some(render_options) => run_render(Path::new(snapshot_path), render_options),
                None => exit_with_usage(),
            }
        }
        _ => exit_with_usage(),
    }
}

fn exit_with_usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

/// What `render` draws and where, from its options.
struct RenderOptions<'a> {
    output_directory: &'a Path,
    koi_body: KoiBody,
    pond_size: PondSize,
    export_scale: u32,
}

/// Read `render`'s options, each a flag and its value, in any order and at most once;
/// the output directory is required. Anything else is a usage error.
fn parse_render_options(option_arguments: &[OsString]) -> Option<RenderOptions<'_>> {
    let mut output_directory = None;
    let mut koi_body = None;
    let mut pond_size = None;
    let mut export_scale = None;
    let (option_pairs, []) = option_arguments.as_chunks::<2>() else {
        return None;
    };
    for [flag, value] in option_pairs {
        let value_text = value.to_str();
        match flag.to_str()? {
            "--output-dir" if output_directory.is_none() => {
                output_directory = Some(Path::new(value));
            }
            "--body" if koi_body.is_none() => koi_body = Some(parse_body(value_text?)?),
            "--size" if pond_size.is_none() => pond_size = Some(value_text?.parse().ok()?),
            "--scale" if export_scale.is_none() => {
                export_scale = Some(
                    value_text?
                        .parse()
                        .ok()
                        .filter(|scale| (1..=MAX_EXPORT_SCALE).contains(scale))?,
                );
            }
            _ => return None,
        }
    }
    Some(RenderOptions {
        output_directory: output_directory?,
        koi_body: koi_body.unwrap_or_default(),
        pond_size: pond_size.unwrap_or_default(),
        export_scale: export_scale.unwrap_or(STATIC_EXPORT_SCALE),
    })
}

fn run_collect(login: &str, output_path: Option<&Path>) -> ExitCode {
    let collection_result = collect_account_snapshot(&GhCli, login, current_utc_date())
        .map_err(|collection_error| collection_error.to_string())
        .and_then(|account_snapshot| {
            serde_json::to_string_pretty(&account_snapshot)
                .map_err(|serialize_error| serialize_error.to_string())
        })
        .and_then(|snapshot_json| write_snapshot(&format!("{snapshot_json}\n"), output_path));
    match collection_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("git-koi: {error_message}");
            ExitCode::FAILURE
        }
    }
}

fn run_render(snapshot_path: &Path, render_options: RenderOptions) -> ExitCode {
    let output_directory = render_options.output_directory;
    let render_result = render_day(snapshot_path, &render_options).and_then(|rendered_day| {
        fs::create_dir_all(output_directory)
            .map_err(|create_error| format!("{}: {create_error}", output_directory.display()))?;
        let mut written_paths = Vec::new();
        for (file_name, file_bytes) in rendered_day.output_files {
            let output_path = output_directory.join(file_name);
            replace_file(&output_path, &file_bytes)?;
            written_paths.push(output_path.display().to_string());
        }
        println!(
            "{}: wrote {}",
            rendered_day.caption,
            written_paths.join(", ")
        );
        Ok(())
    });
    match render_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("git-koi: {error_message}");
            ExitCode::FAILURE
        }
    }
}

/// The day's files, by name, with a caption naming the behavior, observation date, body,
/// and any pond that is not the default.
struct RenderedDay {
    caption: String,
    output_files: [(&'static str, Vec<u8>); 3],
}

/// Render and encode the day's files in memory, so none is written unless all succeed.
fn render_day(snapshot_path: &Path, render_options: &RenderOptions) -> Result<RenderedDay, String> {
    let RenderOptions {
        koi_body,
        pond_size,
        export_scale,
        ..
    } = *render_options;
    let snapshot_json = fs::read_to_string(snapshot_path)
        .map_err(|read_error| format!("{}: {read_error}", snapshot_path.display()))?;
    let account_snapshot: AccountSnapshot = serde_json::from_str(&snapshot_json)
        .map_err(|parse_error| format!("{}: {parse_error}", snapshot_path.display()))?;
    let koi_development = derive_koi_development(&account_snapshot)
        .map_err(|development_error| development_error.to_string())?;
    let behavior = select_behavior(&koi_development);
    let swim_loop = SwimLoop::new(&koi_development, koi_body, behavior, pond_size);
    let swim_frames: Vec<KoiFrame> = (0..swim_loop.frame_count())
        .map(|frame_index| swim_loop.render_frame(frame_index))
        .collect();
    let gif_bytes = encode_gif_scaled(&swim_frames, export_scale)
        .map_err(|encoding_error| encoding_error.to_string())?;
    let png_bytes = encode_png_scaled(
        &render_koi_in(pond_size, &koi_development, koi_body),
        export_scale,
    )
    .map_err(|encoding_error| encoding_error.to_string())?;
    let mut caption = format!(
        "{} for {}, {} body",
        describe_behavior(behavior),
        koi_development.observed_on,
        name_body(koi_body)
    );
    if pond_size != PondSize::default() {
        caption.push_str(&format!(", {pond_size} pond"));
    }
    let preview_html = encode_sized_preview_html(
        &caption,
        pond_size.width_pixels() * export_scale,
        pond_size.height_pixels() * export_scale,
    );
    Ok(RenderedDay {
        caption,
        output_files: [
            ("koi.gif", gif_bytes),
            ("koi.png", png_bytes),
            ("preview.html", preview_html.into_bytes()),
        ],
    })
}

fn parse_body(body_name: &str) -> Option<KoiBody> {
    BODY_NAMES
        .iter()
        .find(|(named_body_name, _)| *named_body_name == body_name)
        .map(|(_, koi_body)| *koi_body)
}

fn name_body(koi_body: KoiBody) -> &'static str {
    BODY_NAMES
        .iter()
        .find(|(_, named_body)| *named_body == koi_body)
        .map(|(body_name, _)| *body_name)
        .expect("every body has a name")
}

fn describe_behavior(behavior: Behavior) -> &'static str {
    match behavior {
        Behavior::DeepRest => "deep rest",
        Behavior::Rest => "rest",
        Behavior::SurfaceFeeding => "surface feeding",
        Behavior::Dart => "dart",
        Behavior::Cruise => "cruise",
    }
}

// GitHub dates its contribution calendar in UTC.
fn current_utc_date() -> NaiveDate {
    let seconds_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock is after 1970")
        .as_secs();
    DateTime::from_timestamp(
        i64::try_from(seconds_since_epoch).expect("the system clock is in range"),
        0,
    )
    .expect("the system clock is in range")
    .date_naive()
}

/// Write to standard output, or replace the file.
fn write_snapshot(snapshot_json: &str, output_path: Option<&Path>) -> Result<(), String> {
    let Some(output_path) = output_path else {
        return io::stdout()
            .write_all(snapshot_json.as_bytes())
            .map_err(|write_error| format!("standard output: {write_error}"));
    };
    replace_file(output_path, snapshot_json.as_bytes())
}

/// Replace the file through a sibling temporary file named after it, so a failed write
/// never leaves a partial file in place.
fn replace_file(output_path: &Path, file_bytes: &[u8]) -> Result<(), String> {
    let mut temporary_name = output_path.file_name().unwrap_or_default().to_owned();
    temporary_name.push(".tmp");
    let temporary_path = output_path.with_file_name(temporary_name);
    fs::write(&temporary_path, file_bytes)
        .and_then(|()| fs::rename(&temporary_path, output_path))
        .map_err(|write_error| {
            let _ = fs::remove_file(&temporary_path);
            format!("{}: {write_error}", output_path.display())
        })
}
