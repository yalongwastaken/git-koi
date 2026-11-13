use chrono::Days;
use git_koi::behavior::Behavior;
use git_koi::coat::KoiBody;
use git_koi::history::AccountSnapshot;
use git_koi::koi::{KoiDevelopment, MAX_GROWTH_CONTRIBUTIONS, derive_koi_development};
use git_koi::motion::{
    calculate_stretched_swim_pose, calculate_swim_pose, frame_count, stretched_frame_count,
};
use git_koi::output::{encode_gif_scaled, encode_png_scaled, encode_sized_preview_html};
use git_koi::pond::PondSize;
use git_koi::render::{KoiFrame, SwimLoop, render_koi, render_koi_in, render_swim_frame};

const HISTORY_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/account-with-history.json"
);
const BEHAVIORS: [Behavior; 5] = [
    Behavior::Cruise,
    Behavior::Dart,
    Behavior::Rest,
    Behavior::SurfaceFeeding,
    Behavior::DeepRest,
];

#[test]
fn the_default_pond_draws_exactly_what_it_always_has() {
    let koi_development = read_fixture_development();
    for behavior in BEHAVIORS {
        let swim_loop = SwimLoop::new(
            &koi_development,
            KoiBody::Asagi,
            behavior,
            PondSize::default(),
        );
        assert_eq!(swim_loop.frame_count(), frame_count(behavior));
        for frame_index in [0, 7, frame_count(behavior) - 1] {
            assert_eq!(
                swim_loop.render_frame(frame_index),
                render_swim_frame(&koi_development, KoiBody::Asagi, behavior, frame_index),
                "{behavior:?} frame {frame_index}"
            );
        }
    }
    assert_eq!(
        render_koi_in(PondSize::default(), &koi_development, KoiBody::Asagi),
        render_koi(&koi_development, KoiBody::Asagi)
    );
}

#[test]
fn ponds_of_every_shape_keep_the_whole_fish_inside() {
    let mut koi_development = read_fixture_development();
    for pond_size in ["768x256", "256x640", "512x512"].map(read_size) {
        for behavior in [
            Behavior::Cruise,
            Behavior::Dart,
            Behavior::Rest,
            Behavior::SurfaceFeeding,
        ] {
            for contribution_total in [0, MAX_GROWTH_CONTRIBUTIONS] {
                koi_development.total_contributions = contribution_total;
                let swim_loop =
                    SwimLoop::new(&koi_development, KoiBody::default(), behavior, pond_size);
                let frame_step = (swim_loop.frame_count() / 6) as usize;
                for frame_index in (0..swim_loop.frame_count()).step_by(frame_step) {
                    let koi_frame = swim_loop.render_frame(frame_index);
                    assert_eq!(koi_frame.pond_size(), pond_size);
                    let [left, top, right, bottom] = measure_opaque_bounds(&koi_frame);
                    assert!(
                        left > 0
                            && top > 0
                            && right < pond_size.width_pixels() - 1
                            && bottom < pond_size.height_pixels() - 1,
                        "{behavior:?} at {contribution_total} contributions leaves the \
                         {pond_size} pond at frame {frame_index}"
                    );
                }
            }
        }
        let still_bounds = measure_opaque_bounds(&render_koi_in(
            pond_size,
            &koi_development,
            KoiBody::default(),
        ));
        let still_center = [
            still_bounds[0] + still_bounds[2],
            still_bounds[1] + still_bounds[3],
        ];
        assert!(still_center[0].abs_diff(pond_size.width_pixels()) <= 2);
        assert!(still_center[1].abs_diff(pond_size.height_pixels()) <= 2);
    }
}

#[test]
fn a_longer_lap_makes_a_longer_loop_that_still_closes_smoothly() {
    let koi_development = read_fixture_development();
    let pond_size = read_size("1200x400");
    let swim_loop = SwimLoop::new(
        &koi_development,
        KoiBody::default(),
        Behavior::Cruise,
        pond_size,
    );
    let loop_frame_count = swim_loop.frame_count();
    let base_frame_count = frame_count(Behavior::Cruise);
    assert!(loop_frame_count > base_frame_count);
    assert_eq!(loop_frame_count % base_frame_count, 0);
    assert_eq!(
        swim_loop.render_frame(loop_frame_count),
        swim_loop.render_frame(0)
    );
    let centroids: Vec<[f64; 2]> = (loop_frame_count - 5..=loop_frame_count + 1)
        .map(|frame_index| measure_opaque_centroid(&swim_loop.render_frame(frame_index)))
        .collect();
    let centroid_steps: Vec<f64> = centroids
        .windows(2)
        .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
        .collect();
    let boundary_step = centroid_steps[4];
    let nearby_average = centroid_steps[..4].iter().sum::<f64>() / 4.0;
    assert!(
        (0.5..1.5).contains(&(boundary_step / nearby_average)),
        "{centroid_steps:?}"
    );
}

#[test]
fn each_day_swims_its_own_track_and_reruns_swim_the_same_one() {
    let koi_development = read_fixture_development();
    let mut next_day = read_fixture_development();
    next_day.observed_on = next_day.observed_on + Days::new(1);
    let pond_size = read_size("768x256");
    let draw = |koi_development: &KoiDevelopment| {
        SwimLoop::new(
            koi_development,
            KoiBody::default(),
            Behavior::Cruise,
            pond_size,
        )
        .render_frame(30)
    };
    assert_eq!(draw(&koi_development), draw(&read_fixture_development()));
    assert_ne!(draw(&koi_development), draw(&next_day));
}

#[test]
fn stretched_loops_keep_every_cycle_closed() {
    for behavior in BEHAVIORS {
        assert_eq!(stretched_frame_count(behavior, 1), frame_count(behavior));
        let stretched_count = stretched_frame_count(behavior, 3);
        let laps = matches!(behavior, Behavior::Cruise | Behavior::Dart);
        assert_eq!(
            stretched_count,
            frame_count(behavior) * if laps { 3 } else { 1 }
        );
        for frame_index in [0, 1, 59, stretched_count - 1] {
            assert_eq!(
                calculate_stretched_swim_pose(behavior, frame_index, 1),
                calculate_swim_pose(behavior, frame_index)
            );
            assert_eq!(
                calculate_stretched_swim_pose(behavior, frame_index + stretched_count, 3),
                calculate_stretched_swim_pose(behavior, frame_index, 3)
            );
        }
    }
}

#[test]
fn exports_match_the_pond_at_the_chosen_scale() {
    let koi_development = read_fixture_development();
    let pond_size = read_size("300x256");
    let koi_frames: Vec<KoiFrame> = (0..3)
        .map(|frame_index| {
            SwimLoop::new(
                &koi_development,
                KoiBody::default(),
                Behavior::Cruise,
                pond_size,
            )
            .render_frame(frame_index)
        })
        .collect();
    for export_scale in [1, 3] {
        let gif_bytes = encode_gif_scaled(&koi_frames, export_scale).unwrap();
        let gif_decoder = gif::DecodeOptions::new()
            .read_info(gif_bytes.as_slice())
            .unwrap();
        assert_eq!(
            [gif_decoder.width(), gif_decoder.height()].map(u32::from),
            [300 * export_scale, 256 * export_scale]
        );
        let png_bytes = encode_png_scaled(&koi_frames[0], export_scale).unwrap();
        let png_reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
            .read_info()
            .unwrap();
        assert_eq!(
            [png_reader.info().width, png_reader.info().height],
            [300 * export_scale, 256 * export_scale]
        );
    }
    let preview_html = encode_sized_preview_html("caption", 300, 256);
    assert_eq!(
        preview_html.matches(r#"width="300" height="256""#).count(),
        4
    );
}

fn read_size(size_text: &str) -> PondSize {
    size_text.parse().unwrap()
}

fn read_fixture_development() -> KoiDevelopment {
    let account_snapshot: AccountSnapshot =
        serde_json::from_str(&std::fs::read_to_string(HISTORY_FIXTURE).unwrap()).unwrap();
    derive_koi_development(&account_snapshot).unwrap()
}

/// The pixels of a frame with their positions, row by row.
fn opaque_positions(koi_frame: &KoiFrame) -> impl Iterator<Item = [u32; 2]> + '_ {
    let width_pixels = koi_frame.pond_size().width_pixels();
    koi_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, pixel_channels)| pixel_channels[3] != 0)
        .map(move |(pixel_index, _)| {
            [
                pixel_index as u32 % width_pixels,
                pixel_index as u32 / width_pixels,
            ]
        })
}

/// `[left, top, right, bottom]` of the drawn pixels, inclusive.
fn measure_opaque_bounds(koi_frame: &KoiFrame) -> [u32; 4] {
    opaque_positions(koi_frame).fold(
        [u32::MAX, u32::MAX, 0, 0],
        |[left, top, right, bottom], [pixel_x, pixel_y]| {
            [
                left.min(pixel_x),
                top.min(pixel_y),
                right.max(pixel_x),
                bottom.max(pixel_y),
            ]
        },
    )
}

fn measure_opaque_centroid(koi_frame: &KoiFrame) -> [f64; 2] {
    let (position_sum, pixel_count) = opaque_positions(koi_frame).fold(
        ([0.0; 2], 0.0),
        |(position_sum, pixel_count), [pixel_x, pixel_y]| {
            (
                [
                    position_sum[0] + f64::from(pixel_x),
                    position_sum[1] + f64::from(pixel_y),
                ],
                pixel_count + 1.0,
            )
        },
    );
    position_sum.map(|sum| sum / pixel_count)
}
