use std::{env, error::Error, fs, path::Path, process::ExitCode};

use git_koi::coat::CoatPattern;
use git_koi::history::AccountSnapshot;
use git_koi::koi::derive_koi_development;
use git_koi::output::{
    STATIC_EXPORT_SCALE, STATIC_HEIGHT_PIXELS, STATIC_WIDTH_PIXELS, encode_png, encode_svg,
};
use git_koi::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, KoiFrame, render_patterned_koi};

const COAT_SAMPLES: [(&str, CoatPattern); 5] = [
    ("kohaku", CoatPattern::Kohaku),
    ("sanke", CoatPattern::Sanke),
    ("showa", CoatPattern::Showa),
    ("tancho", CoatPattern::Tancho),
    ("asagi", CoatPattern::Asagi),
];
const GROWTH_SAMPLES: [(&str, u64); 3] = [("small", 0), ("medium", 1_500), ("large", 12_750)];
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
    for (coat_name, coat_pattern) in COAT_SAMPLES {
        index_markdown.push_str(&format!(
            "## {}\n\n",
            coat_name[..1].to_uppercase() + &coat_name[1..]
        ));
        for (size_name, contribution_total) in GROWTH_SAMPLES {
            koi_development.total_contributions = contribution_total;
            let koi_frame = render_patterned_koi(&koi_development, coat_pattern);
            let file_name = format!("{coat_name}-{size_name}.png");
            write_artifact(output_directory, &file_name, &encode_png(&koi_frame)?)?;
            let svg_name = format!("{coat_name}-{size_name}.svg");
            write_artifact(
                output_directory,
                &svg_name,
                encode_svg(&koi_frame).as_bytes(),
            )?;
            index_markdown.push_str(&format!(
                "![{coat_name} {size_name}]({file_name})\n\n[SVG]({svg_name})\n\n"
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
        These supported coats share the same anatomy, pose, and Classic lighting. The\n\
        samples use a fixed synthetic account seed and explicit growth totals; coat\n\
        selection is not a GitHub language or streak mapping.\n\n\
        Regenerate with the command in the\n\
        [repository README](../README.md#koi-library).\n\n\
        Overview columns run from small to large. Rows follow the coat order below.\n\n\
        [Light overview](overview-light.png) · [Dark overview](overview-dark.png)\n\n\
        Native canvas: {FRAME_WIDTH_PIXELS} × {FRAME_HEIGHT_PIXELS}.\n\n\
        PNG and SVG exports: {STATIC_WIDTH_PIXELS} × {STATIC_HEIGHT_PIXELS}.\n\n\
        Identity seed: {identity_seed}.\n\n"
    );
    for (size_name, contribution_total) in GROWTH_SAMPLES {
        index_markdown.push_str(&format!(
            "- {size_name}: {contribution_total} contributions\n"
        ));
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
    let overview_height = frame_height * COAT_SAMPLES.len() * overview_scale;
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
