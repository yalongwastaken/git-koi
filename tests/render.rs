use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

use chrono::Days;
use git_koi::history::{AccountSnapshot, ContributionDay, LanguageColor};
use git_koi::koi::{
    ADULT_CONTRIBUTIONS, FULL_COLOR_CONTRIBUTIONS, KoiDevelopment, MAX_GROWTH_CONTRIBUTIONS,
    derive_koi_development,
};
use git_koi::output::encode_png;
use git_koi::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, KoiFrame, render_koi};

type LanguageMix<'a> = &'a [(&'a str, Option<&'a str>, f64)];

const RED_MIX: LanguageMix = &[("C++", Some("#f34b7d"), 1.0)];
const FOUR_LANGUAGE_MIX: LanguageMix = &[
    ("C", Some("#555555"), 0.25),
    ("Python", Some("#3572a5"), 0.25),
    ("C++", Some("#f34b7d"), 0.25),
    ("JavaScript", Some("#f1e05a"), 0.25),
];

#[test]
fn language_mixes_color_the_same_fish_reproducibly() {
    let uncolored_frame = render_koi(&develop_with_languages(1_500, &[]));
    let mut distinct_images = BTreeSet::new();
    for language_mix in [
        RED_MIX,
        &[
            ("Python", Some("#3572a5"), 0.65),
            ("C", Some("#555555"), 0.35),
        ],
        &[
            ("TypeScript", Some("#3178c6"), 0.45),
            ("C++", Some("#f34b7d"), 0.35),
            ("Shell", Some("#89e051"), 0.2),
        ],
        FOUR_LANGUAGE_MIX,
        &[("Rust", Some("#dea584"), 1.0)],
    ] {
        let koi_frame = render_koi(&develop_with_languages(1_500, language_mix));
        assert_ne!(koi_frame, uncolored_frame);
        for (uncolored_pixel, colored_pixel) in uncolored_frame
            .as_rgba_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(koi_frame.as_rgba_bytes().as_chunks::<4>().0.iter())
        {
            assert_eq!(uncolored_pixel[3], colored_pixel[3]);
        }
        let png_bytes = encode_png(&koi_frame).unwrap();
        assert_eq!(
            png_bytes,
            encode_png(&render_koi(&develop_with_languages(1_500, language_mix))).unwrap()
        );
        assert!(distinct_images.insert(png_bytes));
    }
}

#[test]
fn languages_on_one_pigment_share_its_patches() {
    let render_mix =
        |language_mix| render_koi(&develop_with_languages(ADULT_CONTRIBUTIONS, language_mix));
    // Python and TypeScript are both blue; C's gray and a missing color are both black.
    assert_eq!(
        render_mix(&[
            ("Python", Some("#3572a5"), 0.5),
            ("TypeScript", Some("#3178c6"), 0.5),
        ]),
        render_mix(&[("Python", Some("#3572a5"), 1.0)])
    );
    assert_eq!(
        render_mix(&[("C", Some("#555555"), 1.0)]),
        render_mix(&[("Unrecognized", None, 1.0)])
    );
    assert_ne!(
        render_mix(&[
            ("Python", Some("#3572a5"), 0.5),
            ("Rust", Some("#dea584"), 0.5),
        ]),
        render_mix(&[("Python", Some("#3572a5"), 1.0)])
    );
}

#[test]
fn equal_pigment_shares_tie_despite_rounding() {
    let render_mix =
        |language_mix| render_koi(&develop_with_languages(ADULT_CONTRIBUTIONS, language_mix));
    // Orange sums to 0.30000000000000004 here; it still ties with yellow, so yellow
    // keeps its place and gives the blue koi its sides.
    assert_eq!(
        render_mix(&[
            ("Python", Some("#3572a5"), 0.4),
            ("Rust", Some("#dea584"), 0.1),
            ("Java", Some("#b07219"), 0.2),
            ("JavaScript", Some("#f1e05a"), 0.3),
        ]),
        render_mix(&[
            ("Python", Some("#3572a5"), 0.4),
            ("Java", Some("#b07219"), 0.3),
            ("JavaScript", Some("#f1e05a"), 0.3),
        ])
    );
}

#[test]
fn the_same_languages_form_different_patches_on_another_account() {
    let mut koi_development = develop_with_languages(ADULT_CONTRIBUTIONS, FOUR_LANGUAGE_MIX);
    let original_frame = render_koi(&koi_development);
    koi_development.identity_seed += 1_u64 << 32;
    assert_ne!(original_frame, render_koi(&koi_development));
}

#[test]
fn young_koi_show_pale_pigments_until_full_color() {
    let collect_mix_colors = |contribution_total, language_mix| {
        collect_opaque_colors(&render_koi(&develop_with_languages(
            contribution_total,
            language_mix,
        )))
    };
    let full_colors = collect_mix_colors(MAX_GROWTH_CONTRIBUTIONS, RED_MIX);
    let red_colors: BTreeSet<_> = full_colors
        .difference(&collect_mix_colors(MAX_GROWTH_CONTRIBUTIONS, &[]))
        .copied()
        .collect();
    let pale_colors: BTreeSet<_> = collect_mix_colors(0, RED_MIX)
        .difference(&collect_mix_colors(0, &[]))
        .filter(|pixel_color| !full_colors.contains(*pixel_color))
        .copied()
        .collect();
    assert!(!pale_colors.is_empty());
    let midway_colors = collect_mix_colors(100, RED_MIX);
    assert!(!midway_colors.is_disjoint(&pale_colors));
    assert!(!midway_colors.is_disjoint(&red_colors));
    for contribution_total in [FULL_COLOR_CONTRIBUTIONS, ADULT_CONTRIBUTIONS] {
        assert!(collect_mix_colors(contribution_total, RED_MIX).is_disjoint(&pale_colors));
    }
}

#[test]
fn growth_slows_and_keeps_the_whole_fish_inside_a_transparent_frame() {
    let mut koi_development = read_fixture_development();
    let mut visible_widths = BTreeMap::new();
    let mut visible_heights = BTreeMap::new();
    let mut area_per_squared_width = BTreeMap::new();
    let mut maximum_frame = None;
    for contribution_total in [
        0,
        500,
        1_000,
        1_500,
        2_000,
        12_750,
        25_000,
        50_000,
        75_000,
        100_000,
        u64::MAX,
    ] {
        koi_development.total_contributions = contribution_total;
        let koi_frame = render_koi(&koi_development);
        let opaque_bounds = measure_opaque_bounds(&koi_frame);
        assert!(opaque_bounds.left_x > 0 && opaque_bounds.top_y > 0);
        assert!(opaque_bounds.right_x < FRAME_WIDTH_PIXELS - 1);
        assert!(opaque_bounds.bottom_y < FRAME_HEIGHT_PIXELS - 1);
        let visible_width_pixels = opaque_bounds.right_x - opaque_bounds.left_x + 1;
        visible_widths.insert(contribution_total, visible_width_pixels);
        visible_heights.insert(
            contribution_total,
            opaque_bounds.bottom_y - opaque_bounds.top_y + 1,
        );
        let opaque_pixel_count = koi_frame
            .as_rgba_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel_channels| pixel_channels[3] != 0)
            .count();
        area_per_squared_width.insert(
            contribution_total,
            opaque_pixel_count as f64 / f64::from(visible_width_pixels).powi(2),
        );
        if contribution_total == 100_000 {
            maximum_frame = Some(koi_frame);
        } else if contribution_total > 100_000 {
            assert_eq!(Some(koi_frame), maximum_frame);
        }
    }
    let growth_widths: Vec<_> = visible_widths.values().copied().collect();
    assert!(
        growth_widths
            .windows(2)
            .all(|adjacent_widths| adjacent_widths[0] <= adjacent_widths[1])
    );
    assert!(
        visible_widths[&500] - visible_widths[&0] > visible_widths[&2_000] - visible_widths[&1_500]
    );
    for contribution_pair in [12_750, 25_000, 50_000, 75_000, 100_000].windows(2) {
        assert!(visible_widths[&contribution_pair[0]] < visible_widths[&contribution_pair[1]]);
        assert!(visible_heights[&contribution_pair[0]] < visible_heights[&contribution_pair[1]]);
    }
    assert!(area_per_squared_width[&100_000] > area_per_squared_width[&12_750]);
}

#[test]
fn png_output_doubles_each_pixel_and_repeats_exactly() {
    let mut koi_development = read_fixture_development();
    koi_development.total_contributions = 1_500;
    let koi_frame = render_koi(&koi_development);
    let png_bytes = encode_png(&koi_frame).unwrap();
    assert_eq!(
        png_bytes,
        encode_png(&render_koi(&koi_development)).unwrap()
    );
    let mut png_reader = png::Decoder::new(Cursor::new(&png_bytes))
        .read_info()
        .unwrap();
    let mut decoded_pixels = vec![0; png_reader.output_buffer_size().unwrap()];
    let frame_info = png_reader.next_frame(&mut decoded_pixels).unwrap();
    assert_eq!((frame_info.width, frame_info.height), (512, 512));
    assert_eq!(frame_info.color_type, png::ColorType::Rgba);
    for (pixel_index, native_channels) in koi_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
    {
        let block_left = pixel_index as u32 % FRAME_WIDTH_PIXELS * 2;
        let block_top = pixel_index as u32 / FRAME_WIDTH_PIXELS * 2;
        for (offset_x, offset_y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let export_index =
                ((block_top + offset_y) * frame_info.width + block_left + offset_x) as usize * 4;
            assert_eq!(
                &decoded_pixels[export_index..export_index + 4],
                native_channels
            );
        }
    }
}

#[test]
fn quiet_days_change_streak_state_without_changing_the_base_fish() {
    let mut account_snapshot: AccountSnapshot =
        serde_json::from_str(include_str!("fixtures/account-with-history.json")).unwrap();
    let active_development = derive_koi_development(&account_snapshot).unwrap();
    let contribution_history = account_snapshot.contribution_history.as_mut().unwrap();
    for day_offset in 1..=14 {
        contribution_history.days.push(ContributionDay {
            date: contribution_history
                .observed_on
                .checked_add_days(Days::new(day_offset))
                .unwrap(),
            contribution_count: 0,
        });
    }
    contribution_history.observed_on = contribution_history.days.last().unwrap().date;
    let quiet_development = derive_koi_development(&account_snapshot).unwrap();
    assert_ne!(active_development.streaks, quiet_development.streaks);
    assert_eq!(
        render_koi(&active_development),
        render_koi(&quiet_development)
    );
}

fn read_fixture_development() -> KoiDevelopment {
    let account_snapshot =
        serde_json::from_str(include_str!("fixtures/account-with-history.json")).unwrap();
    derive_koi_development(&account_snapshot).unwrap()
}

fn develop_with_languages(contribution_total: u64, language_mix: LanguageMix) -> KoiDevelopment {
    let mut koi_development = read_fixture_development();
    koi_development.total_contributions = contribution_total;
    koi_development.language_shares = language_mix
        .iter()
        .map(|&(language_name, _, language_share)| (language_name.to_owned(), language_share))
        .collect();
    koi_development.language_colors = language_mix
        .iter()
        .map(|&(language_name, color_text, _)| {
            (
                language_name.to_owned(),
                color_text.map(|color_text| LanguageColor::try_from(color_text).unwrap()),
            )
        })
        .collect();
    koi_development
}

fn collect_opaque_colors(koi_frame: &KoiFrame) -> BTreeSet<[u8; 4]> {
    koi_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel_channels| pixel_channels[3] != 0)
        .copied()
        .collect()
}

struct OpaqueBounds {
    left_x: u32,
    top_y: u32,
    right_x: u32,
    bottom_y: u32,
}

fn measure_opaque_bounds(koi_frame: &KoiFrame) -> OpaqueBounds {
    let mut opaque_bounds = OpaqueBounds {
        left_x: FRAME_WIDTH_PIXELS,
        top_y: FRAME_HEIGHT_PIXELS,
        right_x: 0,
        bottom_y: 0,
    };
    for (pixel_index, pixel_channels) in koi_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
    {
        if pixel_channels[3] == 0 {
            continue;
        }
        let pixel_x = pixel_index as u32 % FRAME_WIDTH_PIXELS;
        let pixel_y = pixel_index as u32 / FRAME_WIDTH_PIXELS;
        opaque_bounds.left_x = opaque_bounds.left_x.min(pixel_x);
        opaque_bounds.top_y = opaque_bounds.top_y.min(pixel_y);
        opaque_bounds.right_x = opaque_bounds.right_x.max(pixel_x);
        opaque_bounds.bottom_y = opaque_bounds.bottom_y.max(pixel_y);
    }
    assert!(
        opaque_bounds.left_x < opaque_bounds.right_x
            && opaque_bounds.top_y < opaque_bounds.bottom_y
    );
    opaque_bounds
}
