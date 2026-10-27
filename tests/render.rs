use std::collections::BTreeMap;
use std::io::Cursor;

use chrono::Days;
use git_koi::coat::CoatPattern;
use git_koi::history::{AccountSnapshot, ContributionDay};
use git_koi::koi::{KoiDevelopment, derive_koi_development};
use git_koi::output::encode_png;
use git_koi::render::{
    FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, KoiFrame, render_base_koi, render_patterned_koi,
};

#[test]
fn supported_coats_repeat_without_changing_growth_or_transparency() {
    let mut koi_development = read_fixture_development();
    koi_development.total_contributions = 1_500;
    let base_frame = render_base_koi(&koi_development);
    let mut distinct_images = std::collections::BTreeSet::new();
    for coat_pattern in [
        CoatPattern::Kohaku,
        CoatPattern::Sanke,
        CoatPattern::Showa,
        CoatPattern::Tancho,
        CoatPattern::Asagi,
    ] {
        let patterned_frame = render_patterned_koi(&koi_development, coat_pattern);
        assert_ne!(patterned_frame, base_frame);
        for (base_pixel, patterned_pixel) in base_frame
            .as_rgba_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(patterned_frame.as_rgba_bytes().as_chunks::<4>().0.iter())
        {
            assert_eq!(base_pixel[3], patterned_pixel[3]);
        }
        let png_bytes = encode_png(&patterned_frame).unwrap();
        assert_eq!(
            png_bytes,
            encode_png(&render_patterned_koi(&koi_development, coat_pattern)).unwrap()
        );
        assert!(distinct_images.insert(png_bytes));
    }
}

#[test]
fn patch_rendering_uses_the_full_account_identity() {
    let mut koi_development = read_fixture_development();
    koi_development.total_contributions = 1_500;
    let original_frame = render_patterned_koi(&koi_development, CoatPattern::Sanke);
    koi_development.identity_seed += 1_u64 << 32;
    assert_ne!(
        original_frame,
        render_patterned_koi(&koi_development, CoatPattern::Sanke)
    );
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
        let koi_frame = render_base_koi(&koi_development);
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
    let koi_frame = render_patterned_koi(&koi_development, CoatPattern::Sanke);
    let png_bytes = encode_png(&koi_frame).unwrap();
    assert_eq!(
        png_bytes,
        encode_png(&render_patterned_koi(&koi_development, CoatPattern::Sanke)).unwrap()
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
        render_base_koi(&active_development),
        render_base_koi(&quiet_development)
    );
}

fn read_fixture_development() -> KoiDevelopment {
    let account_snapshot =
        serde_json::from_str(include_str!("fixtures/account-with-history.json")).unwrap();
    derive_koi_development(&account_snapshot).unwrap()
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
