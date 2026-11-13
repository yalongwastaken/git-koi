use std::{env, error::Error, fs, path::Path, process::ExitCode};

use chrono::{Datelike, Days, NaiveDate};
use git_koi::behavior::Behavior;
use git_koi::coat::KoiBody;
use git_koi::history::{AccountSnapshot, ContributionWeek, DayActivity, LanguageColor};
use git_koi::koi::{
    ADULT_CONTRIBUTIONS, KoiDevelopment, MAX_GROWTH_CONTRIBUTIONS, RecentStreak,
    YOUNG_SHAPE_CONTRIBUTIONS, derive_koi_development,
};
use git_koi::motion::{calculate_swim_pose, frame_count};
use git_koi::output::{
    STATIC_EXPORT_SCALE, STATIC_HEIGHT_PIXELS, STATIC_WIDTH_PIXELS, encode_png, encode_svg,
};
use git_koi::render::{
    FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, KoiFrame, render_koi, render_swim_frame,
};

/// Language mixes with GitHub's colors and their shares of the account's projects.
type LanguageMix = &'static [(&'static str, &'static str, f64)];
const LANGUAGE_SAMPLES: [(&str, LanguageMix); 4] = [
    ("one", &[("C++", "#f34b7d", 1.0)]),
    (
        "two",
        &[("Python", "#3572a5", 0.65), ("C", "#555555", 0.35)],
    ),
    (
        "three",
        &[
            ("TypeScript", "#3178c6", 0.45),
            ("C++", "#f34b7d", 0.35),
            ("Shell", "#89e051", 0.2),
        ],
    ),
    (
        "four",
        &[
            ("C", "#555555", 0.25),
            ("Python", "#3572a5", 0.25),
            ("C++", "#f34b7d", 0.25),
            ("JavaScript", "#f1e05a", 0.25),
        ],
    ),
];
/// One language for each pigment, so every pigment shows on every body.
const BODY_MIX: LanguageMix = &[
    ("C++", "#f34b7d", 0.2),
    ("C", "#555555", 0.2),
    ("JavaScript", "#f1e05a", 0.2),
    ("Rust", "#dea584", 0.2),
    ("Python", "#3572a5", 0.2),
];
const BODY_SAMPLES: [(&str, KoiBody); 6] = [
    ("white", KoiBody::White),
    ("black", KoiBody::Black),
    ("asagi", KoiBody::Asagi),
    ("tea", KoiBody::Tea),
    ("gold", KoiBody::Gold),
    ("platinum", KoiBody::Platinum),
];
/// A recent streak sample: still running, or ended some days before the observation.
#[derive(Clone, Copy)]
enum RecentSample {
    Current { weeks: usize },
    Ended { weeks: usize, days_since_end: u64 },
}
/// Streak samples on the four-language mix: name, longest streak in weeks, and the
/// recent streak. Each row of six fills one overview row.
const MARKING_SAMPLES: [(&str, usize, Option<RecentSample>); 12] = [
    (
        "current-1-week",
        1,
        Some(RecentSample::Current { weeks: 1 }),
    ),
    (
        "current-6-weeks",
        6,
        Some(RecentSample::Current { weeks: 6 }),
    ),
    (
        "current-13-weeks",
        13,
        Some(RecentSample::Current { weeks: 13 }),
    ),
    ("ended-1-week-ago", 13, Some(ended_long_streak(7))),
    ("ended-2-weeks-ago", 13, Some(ended_long_streak(14))),
    ("ended-4-weeks-ago", 13, Some(ended_long_streak(28))),
    ("longest-0-weeks", 0, None),
    ("longest-1-week", 1, Some(ended_long_ago(1))),
    ("longest-4-weeks", 4, Some(ended_long_ago(4))),
    ("longest-52-weeks", 52, Some(ended_long_ago(52))),
    ("longest-104-weeks", 104, Some(ended_long_ago(104))),
    ("longest-208-weeks", 208, Some(ended_long_ago(208))),
];
/// One pond and water sample: name, issues and pull requests opened on each of the
/// last seven days oldest first, contributions in the week, and contributions on the
/// observation date.
struct PondSample {
    name: &'static str,
    issues_per_day: [u32; 7],
    pull_requests_per_day: [u32; 7],
    week_contributions: u64,
    day_contributions: u64,
}

const fn pond_sample(
    name: &'static str,
    issues_per_day: [u32; 7],
    pull_requests_per_day: [u32; 7],
    week_contributions: u64,
    day_contributions: u64,
) -> PondSample {
    PondSample {
        name,
        issues_per_day,
        pull_requests_per_day,
        week_contributions,
        day_contributions,
    }
}

/// Middling water for the rows that show the objects.
const SAMPLE_WEEK_CONTRIBUTIONS: u64 = 20;
const SAMPLE_DAY_CONTRIBUTIONS: u32 = 3;
/// Pond samples, each row of six filling one overview row: lily pads by the day's issue
/// count, rocks by the day's pull request count, lily pads by days with an issue, rocks
/// by days with a pull request, water marks by the week's contributions, and the wake
/// by the day's contributions.
const POND_SAMPLES: [PondSample; 36] = [
    pond_sample("0-issues-yesterday", [0; 7], [0; 7], 20, 3),
    pond_sample("1-issue-yesterday", [0, 0, 0, 0, 0, 0, 1], [0; 7], 20, 3),
    pond_sample("2-issues-yesterday", [0, 0, 0, 0, 0, 0, 2], [0; 7], 20, 3),
    pond_sample("3-issues-yesterday", [0, 0, 0, 0, 0, 0, 3], [0; 7], 20, 3),
    pond_sample("4-issues-yesterday", [0, 0, 0, 0, 0, 0, 4], [0; 7], 20, 3),
    pond_sample("8-issues-yesterday", [0, 0, 0, 0, 0, 0, 8], [0; 7], 20, 3),
    pond_sample("0-pull-requests-yesterday", [0; 7], [0; 7], 20, 3),
    pond_sample(
        "1-pull-request-yesterday",
        [0; 7],
        [0, 0, 0, 0, 0, 0, 1],
        20,
        3,
    ),
    pond_sample(
        "2-pull-requests-yesterday",
        [0; 7],
        [0, 0, 0, 0, 0, 0, 2],
        20,
        3,
    ),
    pond_sample(
        "3-pull-requests-yesterday",
        [0; 7],
        [0, 0, 0, 0, 0, 0, 3],
        20,
        3,
    ),
    pond_sample(
        "4-pull-requests-yesterday",
        [0; 7],
        [0, 0, 0, 0, 0, 0, 4],
        20,
        3,
    ),
    pond_sample(
        "8-pull-requests-yesterday",
        [0; 7],
        [0, 0, 0, 0, 0, 0, 8],
        20,
        3,
    ),
    pond_sample("1-issue-day", [0, 0, 0, 0, 0, 0, 1], [0; 7], 20, 3),
    pond_sample("2-issue-days", [0, 0, 0, 0, 0, 1, 1], [0; 7], 20, 3),
    pond_sample("3-issue-days", [0, 0, 0, 0, 1, 1, 1], [0; 7], 20, 3),
    pond_sample("4-issue-days", [0, 0, 0, 1, 1, 1, 1], [0; 7], 20, 3),
    pond_sample("5-issue-days", [0, 0, 1, 1, 1, 1, 1], [0; 7], 20, 3),
    pond_sample("7-issue-days", [1; 7], [0; 7], 20, 3),
    pond_sample("1-pull-request-day", [0; 7], [0, 0, 0, 0, 0, 0, 1], 20, 3),
    pond_sample("2-pull-request-days", [0; 7], [0, 0, 0, 0, 0, 1, 1], 20, 3),
    pond_sample("3-pull-request-days", [0; 7], [0, 0, 0, 0, 1, 1, 1], 20, 3),
    pond_sample("4-pull-request-days", [0; 7], [0, 0, 0, 1, 1, 1, 1], 20, 3),
    pond_sample("5-pull-request-days", [0; 7], [0, 0, 1, 1, 1, 1, 1], 20, 3),
    pond_sample("7-pull-request-days", [0; 7], [1; 7], 20, 3),
    pond_sample("0-contributions-this-week", [0; 7], [0; 7], 0, 3),
    pond_sample("1-contribution-this-week", [0; 7], [0; 7], 1, 3),
    pond_sample("5-contributions-this-week", [0; 7], [0; 7], 5, 3),
    pond_sample("20-contributions-this-week", [0; 7], [0; 7], 20, 3),
    pond_sample("50-contributions-this-week", [0; 7], [0; 7], 50, 3),
    pond_sample("100-contributions-this-week", [0; 7], [0; 7], 100, 3),
    pond_sample("0-contributions-yesterday", [0; 7], [0; 7], 20, 0),
    pond_sample("1-contribution-yesterday", [0; 7], [0; 7], 20, 1),
    pond_sample("3-contributions-yesterday", [0; 7], [0; 7], 20, 3),
    pond_sample("10-contributions-yesterday", [0; 7], [0; 7], 20, 10),
    pond_sample("30-contributions-yesterday", [0; 7], [0; 7], 20, 30),
    pond_sample("100-contributions-yesterday", [0; 7], [0; 7], 20, 100),
];
const GROWTH_SAMPLES: [u64; 6] = [
    0,
    500,
    YOUNG_SHAPE_CONTRIBUTIONS,
    ADULT_CONTRIBUTIONS,
    50_000,
    MAX_GROWTH_CONTRIBUTIONS,
];
const USAGE: &str = "Usage: generate-library OUTPUT_DIRECTORY
Write reproducible PNGs, SVGs, and an index; replace matching files, preserve others.
Success is silent. Errors go to stderr and exit 1; invalid usage exits 2.
An interrupted run may leave a partial library; rerun the same command to repair it.
Example: cargo run --offline --locked --example generate-library -- library";

fn main() -> ExitCode {
    let program_arguments: Vec<_> = env::args_os().skip(1).collect();
    if program_arguments.len() == 1 && program_arguments[0] == "--help" {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if program_arguments.len() != 1 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let output_directory = Path::new(&program_arguments[0]);
    match generate_library(output_directory) {
        Ok(()) => ExitCode::SUCCESS,
        Err(generation_error) => {
            eprintln!("{}: {generation_error}", output_directory.display());
            ExitCode::FAILURE
        }
    }
}

fn generate_library(output_directory: &Path) -> Result<(), Box<dyn Error>> {
    let account_snapshot: AccountSnapshot =
        serde_json::from_str(include_str!("../tests/fixtures/account-with-history.json"))?;
    let mut koi_development = derive_koi_development(&account_snapshot)?;
    // Growth samples show normal colors and no streak fins.
    set_streaks(&mut koi_development, 4, Some(ended_long_ago(4)));
    let mut index_markdown = build_index_header(koi_development.identity_seed);
    fs::create_dir_all(output_directory)?;
    let mut mix_frames = Vec::new();
    for (mix_name, language_mix) in LANGUAGE_SAMPLES {
        let mix_description = apply_language_mix(&mut koi_development, language_mix)?;
        index_markdown.push_str(&format!(
            "## {}\n\n{mix_description}.\n\n",
            capitalize(mix_name)
        ));
        mix_frames.extend(render_growth_samples(
            output_directory,
            &mut koi_development,
            KoiBody::default(),
            mix_name,
            &mut index_markdown,
        )?);
    }
    write_overviews(output_directory, "overview", &mix_frames)?;
    let body_mix_description = apply_language_mix(&mut koi_development, BODY_MIX)?;
    let mut body_frames = Vec::new();
    for (body_name, koi_body) in BODY_SAMPLES {
        index_markdown.push_str(&format!(
            "## {} Body\n\n{body_mix_description}.\n\n",
            capitalize(body_name)
        ));
        body_frames.extend(render_growth_samples(
            output_directory,
            &mut koi_development,
            koi_body,
            &format!("body-{body_name}"),
            &mut index_markdown,
        )?);
    }
    write_overviews(output_directory, "bodies", &body_frames)?;
    let mut marking_frames = Vec::new();
    koi_development.total_contributions = ADULT_CONTRIBUTIONS;
    let marking_mix_description = apply_language_mix(&mut koi_development, LANGUAGE_SAMPLES[3].1)?;
    index_markdown.push_str(&format!(
        "## Streak Markings\n\n{marking_mix_description}, at {ADULT_CONTRIBUTIONS} contributions.\n\n"
    ));
    for (sample_name, longest_week_count, recent_sample) in MARKING_SAMPLES {
        set_streaks(&mut koi_development, longest_week_count, recent_sample);
        marking_frames.push(render_sample(
            output_directory,
            &koi_development,
            KoiBody::default(),
            &format!("marks-{sample_name}"),
            &capitalize(&sample_name.replace('-', " ")),
            &mut index_markdown,
        )?);
    }
    apply_language_mix(&mut koi_development, BODY_MIX)?;
    index_markdown.push_str(&format!(
        "## Bodies With and Without Marks\n\n{body_mix_description}, at \
        {ADULT_CONTRIBUTIONS} contributions. Without marks, no streak has happened yet; \
        with every mark, a 13-week streak is running and the longest is 208 weeks.\n\n"
    ));
    for (mark_name, longest_week_count, recent_sample) in [
        ("unmarked", 0, None),
        ("marked", 208, Some(RecentSample::Current { weeks: 13 })),
    ] {
        set_streaks(&mut koi_development, longest_week_count, recent_sample);
        for (body_name, koi_body) in BODY_SAMPLES {
            marking_frames.push(render_sample(
                output_directory,
                &koi_development,
                koi_body,
                &format!("marks-{mark_name}-{body_name}"),
                &format!("{} Body, {}", capitalize(body_name), capitalize(mark_name)),
                &mut index_markdown,
            )?);
        }
    }
    write_overviews(output_directory, "markings", &marking_frames)?;
    set_streaks(&mut koi_development, 4, Some(ended_long_ago(4)));
    index_markdown.push_str(&format!(
        "## Pond and Water\n\n{body_mix_description}, cruising at the peak of a stroke so the \
        water shows. The bodies sheet with the pond added: each row is one body across the \
        contribution totals, and adds one element of the pond rising with them. Lily pads by \
        the day's issue count, rocks by the day's pull request count, lily pads by the number \
        of days with an issue, rocks by the number of days with a pull request, water marks \
        by the week's contributions, and the wake by the day's contributions. The object rows \
        use {SAMPLE_WEEK_CONTRIBUTIONS} contributions in the week and \
        {SAMPLE_DAY_CONTRIBUTIONS} on the day.\n\n"
    ));
    let stroke_peak_frame = (0..frame_count(Behavior::Cruise))
        .max_by(|&first, &second| {
            calculate_swim_pose(Behavior::Cruise, first)
                .wake_strength
                .total_cmp(&calculate_swim_pose(Behavior::Cruise, second).wake_strength)
        })
        .expect("a cruise has frames");
    let mut pond_frames = Vec::new();
    for (sample_index, pond_sample) in POND_SAMPLES.iter().enumerate() {
        let (body_name, koi_body) = BODY_SAMPLES[sample_index / GROWTH_SAMPLES.len()];
        let contribution_total = GROWTH_SAMPLES[sample_index % GROWTH_SAMPLES.len()];
        koi_development.total_contributions = contribution_total;
        set_week(
            &mut koi_development,
            pond_sample.issues_per_day,
            pond_sample.pull_requests_per_day,
        );
        koi_development.recent_activity.recent_contributions = pond_sample.week_contributions;
        koi_development
            .recent_activity
            .observation_week_contributions = vec![pond_sample.day_contributions];
        pond_frames.push(write_sample(
            output_directory,
            render_swim_frame(
                &koi_development,
                koi_body,
                Behavior::Cruise,
                stroke_peak_frame,
            ),
            &format!("pond-{body_name}-{}", pond_sample.name),
            &format!(
                "{} Body, {contribution_total} Contributions, {}",
                capitalize(body_name),
                pond_sample.name.replace('-', " ")
            ),
            &mut index_markdown,
        )?);
    }
    write_overviews(output_directory, "pond", &pond_frames)?;
    write_artifact(
        output_directory,
        "README.md",
        format!("{}\n", index_markdown.trim_end()).as_bytes(),
    )?;
    Ok(())
}

/// Replace the development's languages with a sample mix and describe it for the index.
fn apply_language_mix(
    koi_development: &mut KoiDevelopment,
    language_mix: LanguageMix,
) -> Result<String, Box<dyn Error>> {
    koi_development.language_shares.clear();
    koi_development.language_colors.clear();
    let mut mix_description = Vec::new();
    for &(language_name, color_text, language_share) in language_mix {
        koi_development
            .language_shares
            .insert(language_name.to_owned(), language_share);
        koi_development.language_colors.insert(
            language_name.to_owned(),
            Some(LanguageColor::try_from(color_text)?),
        );
        mix_description.push(format!(
            "{language_name} {:.0}% ({color_text})",
            language_share * 100.0
        ));
    }
    Ok(mix_description.join(", "))
}

/// Replace the development's streaks with a sample longest and recent streak.
fn set_streaks(
    koi_development: &mut KoiDevelopment,
    longest_week_count: usize,
    recent_sample: Option<RecentSample>,
) {
    koi_development.streaks.longest = build_weeks(longest_week_count);
    koi_development.streaks.recent = recent_sample.map(|recent_sample| match recent_sample {
        RecentSample::Current { weeks } => RecentStreak::Current(build_weeks(weeks)),
        RecentSample::Ended {
            weeks,
            days_since_end,
        } => RecentStreak::Ended {
            weeks: build_weeks(weeks),
            ended_on: koi_development.observed_on - Days::new(days_since_end),
        },
    });
}

/// Replace the development's week with the given counts, oldest first, ending on the
/// observation date.
fn set_week(
    koi_development: &mut KoiDevelopment,
    issues_per_day: [u32; 7],
    pull_requests_per_day: [u32; 7],
) {
    koi_development.daily_activity = (0..7)
        .map(|day_index| DayActivity {
            date: koi_development.observed_on - Days::new(6 - day_index as u64),
            issues_opened: issues_per_day[day_index],
            pull_requests_opened: pull_requests_per_day[day_index],
        })
        .collect();
}

/// A 13-week streak that ended some days ago, so its fins are fading.
const fn ended_long_streak(days_since_end: u64) -> RecentSample {
    RecentSample::Ended {
        weeks: 13,
        days_since_end,
    }
}

/// A streak that ended long enough ago that its fins have faded.
const fn ended_long_ago(weeks: usize) -> RecentSample {
    RecentSample::Ended {
        weeks,
        days_since_end: 70,
    }
}

/// Fully active weeks; the marks depend only on how many there are.
fn build_weeks(week_count: usize) -> Vec<ContributionWeek> {
    let first_monday: NaiveDate = "2024-01-01".parse().expect("a valid date");
    (0..week_count)
        .map(|week_index| ContributionWeek {
            iso_week: (first_monday + Days::new(7 * week_index as u64)).iso_week(),
            active_day_count: 7,
            observed_day_count: 7,
        })
        .collect()
}

/// Render and write one sample per growth total, and list them in the index.
fn render_growth_samples(
    output_directory: &Path,
    koi_development: &mut KoiDevelopment,
    koi_body: KoiBody,
    sample_name: &str,
    index_markdown: &mut String,
) -> Result<Vec<KoiFrame>, Box<dyn Error>> {
    let mut rendered_frames = Vec::new();
    for contribution_total in GROWTH_SAMPLES {
        koi_development.total_contributions = contribution_total;
        rendered_frames.push(render_sample(
            output_directory,
            koi_development,
            koi_body,
            &format!("{sample_name}-{contribution_total}"),
            &format!("{contribution_total} Contributions"),
            index_markdown,
        )?);
    }
    Ok(rendered_frames)
}

/// Render the still and write it as one PNG/SVG pair, listed in the index under a
/// heading.
fn render_sample(
    output_directory: &Path,
    koi_development: &KoiDevelopment,
    koi_body: KoiBody,
    file_stem: &str,
    heading: &str,
    index_markdown: &mut String,
) -> Result<KoiFrame, Box<dyn Error>> {
    write_sample(
        output_directory,
        render_koi(koi_development, koi_body),
        file_stem,
        heading,
        index_markdown,
    )
}

/// Write one frame as a PNG/SVG pair, and list it in the index under a heading.
fn write_sample(
    output_directory: &Path,
    koi_frame: KoiFrame,
    file_stem: &str,
    heading: &str,
    index_markdown: &mut String,
) -> Result<KoiFrame, Box<dyn Error>> {
    let file_name = format!("{file_stem}.png");
    write_artifact(output_directory, &file_name, &encode_png(&koi_frame)?)?;
    let svg_name = format!("{file_stem}.svg");
    write_artifact(
        output_directory,
        &svg_name,
        encode_svg(&koi_frame).as_bytes(),
    )?;
    index_markdown.push_str(&format!(
        "### {heading}\n\n![{file_stem}]({file_name})\n\n[SVG]({svg_name})\n\n"
    ));
    Ok(koi_frame)
}

fn write_overviews(
    output_directory: &Path,
    overview_name: &str,
    rendered_frames: &[KoiFrame],
) -> Result<(), Box<dyn Error>> {
    for (background_name, background_color) in
        [("light", [255, 255, 255, 255]), ("dark", [13, 17, 23, 255])]
    {
        write_artifact(
            output_directory,
            &format!("{overview_name}-{background_name}.png"),
            &encode_overview(rendered_frames, background_color)?,
        )?;
    }
    Ok(())
}

fn capitalize(sample_name: &str) -> String {
    sample_name[..1].to_uppercase() + &sample_name[1..]
}

fn build_index_header(identity_seed: u64) -> String {
    let mut index_markdown = format!(
        "# Koi Library\n\n\
        These samples share the same anatomy, pose, and Classic lighting. Language mixes\n\
        appear on the default white body, followed by each body with one language per\n\
        pigment, then streak markings. The samples use a fixed synthetic account seed,\n\
        explicit language shares with their GitHub colors, explicit growth totals, and\n\
        explicit streaks. Growth samples show normal colors and no streak fins.\n\n\
        Regenerate with the command in the\n\
        [repository README](../README.md#koi-library).\n\n\
        In the mix and body overviews, columns follow the contribution totals below,\n\
        from left to right, and rows follow the mix or body order. These are samples of\n\
        continuous growth. The markings overview follows the streak marking sections in\n\
        rows of six: current streaks, longest streaks, then bodies without and with\n\
        marks. The pond and water overview is the bodies overview with the pond added:\n\
        the same bodies down the rows and contribution totals across the columns, and\n\
        each row adds one element rising with them: lily pads by the day's issues,\n\
        rocks by the day's pull requests, lily pads by days with an issue, rocks by days\n\
        with a pull request, water marks by the week's contributions, and the wake by\n\
        the day's contributions.\n\n\
        Mixes: [light overview](overview-light.png) · [dark overview](overview-dark.png)\n\n\
        Bodies: [light overview](bodies-light.png) · [dark overview](bodies-dark.png)\n\n\
        Streak markings: [light overview](markings-light.png) ·\n\
        [dark overview](markings-dark.png)\n\n\
        Pond and water: [light overview](pond-light.png) ·\n\
        [dark overview](pond-dark.png)\n\n\
        Native canvas: {FRAME_WIDTH_PIXELS} × {FRAME_HEIGHT_PIXELS}.\n\n\
        PNG and SVG exports: {STATIC_WIDTH_PIXELS} × {STATIC_HEIGHT_PIXELS}.\n\n\
        Identity seed: {identity_seed}.\n\n"
    );
    for contribution_total in GROWTH_SAMPLES {
        index_markdown.push_str(&format!("- {contribution_total} contributions\n"));
    }
    index_markdown.push('\n');
    index_markdown
}

fn encode_overview(
    rendered_frames: &[KoiFrame],
    background_color: [u8; 4],
) -> Result<Vec<u8>, png::EncodingError> {
    let overview_scale = STATIC_EXPORT_SCALE as usize;
    let frame_width = FRAME_WIDTH_PIXELS as usize;
    let frame_height = FRAME_HEIGHT_PIXELS as usize;
    let overview_width = frame_width * GROWTH_SAMPLES.len() * overview_scale;
    let overview_height =
        frame_height * rendered_frames.len() / GROWTH_SAMPLES.len() * overview_scale;
    let mut overview_pixels = background_color.repeat(overview_width * overview_height);
    for (pixel_index, pixel_channels) in overview_pixels
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let source_x = pixel_index % overview_width / overview_scale;
        let source_y = pixel_index / overview_width / overview_scale;
        let frame_index = source_y / frame_height * GROWTH_SAMPLES.len() + source_x / frame_width;
        let source_index = (source_y % frame_height * frame_width + source_x % frame_width) * 4;
        let source_channels =
            &rendered_frames[frame_index].as_rgba_bytes()[source_index..source_index + 4];
        if source_channels[3] != 0 {
            pixel_channels.copy_from_slice(source_channels);
        }
    }
    let mut png_bytes = Vec::new();
    let mut png_encoder = png::Encoder::new(
        &mut png_bytes,
        overview_width as u32,
        overview_height as u32,
    );
    png_encoder.set_color(png::ColorType::Rgba);
    png_encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = png_encoder.write_header()?;
    png_writer.write_image_data(&overview_pixels)?;
    png_writer.finish()?;
    Ok(png_bytes)
}

fn write_artifact(
    output_directory: &Path,
    file_name: &str,
    file_bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    let output_path = output_directory.join(file_name);
    let temporary_path = output_path.with_extension("tmp");
    fs::write(&temporary_path, file_bytes)
        .map_err(|write_error| format!("{file_name}: {write_error}"))?;
    fs::rename(temporary_path, output_path)
        .map_err(|rename_error| format!("{file_name}: {rename_error}"))?;
    Ok(())
}
