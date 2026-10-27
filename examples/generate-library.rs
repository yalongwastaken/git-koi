use std::{env, error::Error, fs, path::Path, process::ExitCode};

use git_koi::coat::KoiBody;
use git_koi::history::{AccountSnapshot, LanguageColor};
use git_koi::koi::{
    ADULT_CONTRIBUTIONS, KoiDevelopment, MAX_GROWTH_CONTRIBUTIONS, YOUNG_SHAPE_CONTRIBUTIONS,
    derive_koi_development,
};
use git_koi::output::{
    STATIC_EXPORT_SCALE, STATIC_HEIGHT_PIXELS, STATIC_WIDTH_PIXELS, encode_png, encode_svg,
};
use git_koi::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, KoiFrame, render_koi};

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
        let koi_frame = render_koi(koi_development, koi_body);
        let file_name = format!("{sample_name}-{contribution_total}.png");
        write_artifact(output_directory, &file_name, &encode_png(&koi_frame)?)?;
        let svg_name = format!("{sample_name}-{contribution_total}.svg");
        write_artifact(
            output_directory,
            &svg_name,
            encode_svg(&koi_frame).as_bytes(),
        )?;
        index_markdown.push_str(&format!(
            "### {contribution_total} Contributions\n\n\
            ![{sample_name} at {contribution_total} contributions]({file_name})\n\n\
            [SVG]({svg_name})\n\n"
        ));
        rendered_frames.push(koi_frame);
    }
    Ok(rendered_frames)
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
        pigment. The samples use a fixed synthetic account seed, explicit language shares\n\
        with their GitHub colors, and explicit growth totals.\n\n\
        Regenerate with the command in the\n\
        [repository README](../README.md#koi-library).\n\n\
        Overview columns follow the contribution totals below, from left to right. Rows\n\
        follow the mix or body order. These are samples of continuous growth.\n\n\
        Mixes: [light overview](overview-light.png) · [dark overview](overview-dark.png)\n\n\
        Bodies: [light overview](bodies-light.png) · [dark overview](bodies-dark.png)\n\n\
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
