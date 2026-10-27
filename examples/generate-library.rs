use std::{env, error::Error, fs, path::Path, process::ExitCode};

use git_koi::history::{AccountSnapshot, LanguageColor};
use git_koi::koi::{
    ADULT_CONTRIBUTIONS, MAX_GROWTH_CONTRIBUTIONS, YOUNG_SHAPE_CONTRIBUTIONS,
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
    let mut rendered_frames = Vec::new();
    fs::create_dir_all(output_directory)?;
    for (mix_name, language_mix) in LANGUAGE_SAMPLES {
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
        index_markdown.push_str(&format!(
            "## {}\n\n{}.\n\n",
            mix_name[..1].to_uppercase() + &mix_name[1..],
            mix_description.join(", ")
        ));
        for contribution_total in GROWTH_SAMPLES {
            koi_development.total_contributions = contribution_total;
            let koi_frame = render_koi(&koi_development);
            let file_name = format!("{mix_name}-{contribution_total}.png");
            write_artifact(output_directory, &file_name, &encode_png(&koi_frame)?)?;
            let svg_name = format!("{mix_name}-{contribution_total}.svg");
            write_artifact(
                output_directory,
                &svg_name,
                encode_svg(&koi_frame).as_bytes(),
            )?;
            index_markdown.push_str(&format!(
                "### {contribution_total} Contributions\n\n\
                ![{mix_name} at {contribution_total} contributions]({file_name})\n\n\
                [SVG]({svg_name})\n\n"
            ));
            rendered_frames.push(koi_frame);
        }
    }
    for (background_name, background_color) in
        [("light", [255, 255, 255, 255]), ("dark", [13, 17, 23, 255])]
    {
        write_artifact(
            output_directory,
            &format!("overview-{background_name}.png"),
            &encode_overview(&rendered_frames, background_color)?,
        )?;
    }
    write_artifact(
        output_directory,
        "README.md",
        format!("{}\n", index_markdown.trim_end()).as_bytes(),
    )?;
    Ok(())
}

fn build_index_header(identity_seed: u64) -> String {
    let mut index_markdown = format!(
        "# Koi Library\n\n\
        These language mixes share the same anatomy, pose, and Classic lighting. The\n\
        samples use a fixed synthetic account seed, explicit language shares with their\n\
        GitHub colors, and explicit growth totals.\n\n\
        Regenerate with the command in the\n\
        [repository README](../README.md#koi-library).\n\n\
        Overview columns follow the contribution totals below, from left to right. Rows\n\
        follow the mix order. These are samples of continuous growth.\n\n\
        [Light overview](overview-light.png) · [Dark overview](overview-dark.png)\n\n\
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
    let overview_height = frame_height * LANGUAGE_SAMPLES.len() * overview_scale;
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
