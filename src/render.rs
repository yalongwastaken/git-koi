//! A curved koi with Classic shading and transparent surroundings.

use std::f64::consts::{PI, TAU};

use crate::coat::{CoatPattern, WHITE_RAMP, select_body_ramp, select_fin_ramp};
use crate::koi::{KoiDevelopment, calculate_growth_fraction};

pub const FRAME_WIDTH_PIXELS: u32 = 256;
pub const FRAME_HEIGHT_PIXELS: u32 = FRAME_WIDTH_PIXELS;

/// A renderer-owned, fixed-size frame with row-major RGBA pixels.
#[derive(Debug, Eq, PartialEq)]
pub struct KoiFrame {
    rgba_pixels: Vec<[u8; 4]>,
}

impl KoiFrame {
    pub fn as_rgba_bytes(&self) -> &[u8] {
        self.rgba_pixels.as_flattened()
    }
}

/// Render contribution-driven growth in a centered, fixed swimming pose.
/// Language and streak markings are not painted by this base renderer. Identical
/// inputs produce identical pixels without I/O, clock reads, or input mutation.
pub fn render_base_koi(koi_development: &KoiDevelopment) -> KoiFrame {
    render_patterned_koi(koi_development, CoatPattern::Plain)
}

/// Render a supported coat on the same anatomy and pose as the base koi.
/// The account seed shapes patch edges in body coordinates, so growth does not
/// reroll the pattern. The caller selects the coat; languages and streaks do not
/// select or modify it. Identical inputs produce identical pixels without I/O.
pub fn render_patterned_koi(
    koi_development: &KoiDevelopment,
    coat_pattern: CoatPattern,
) -> KoiFrame {
    const MIN_BODY_LENGTH_PIXELS: f64 = 30.0;
    const MAX_BODY_LENGTH_PIXELS: f64 = 110.0;
    let body_length_pixels = MIN_BODY_LENGTH_PIXELS
        + (MAX_BODY_LENGTH_PIXELS - MIN_BODY_LENGTH_PIXELS)
            * calculate_growth_fraction(koi_development.total_contributions);
    let body_spine = build_spine(body_length_pixels);
    let fin_curves = build_fins(&body_spine, body_length_pixels);
    let barbel_curves = build_barbels(&body_spine);
    let pose_center = calculate_pose_center(&body_spine, &fin_curves, &barbel_curves);
    let koi_appearance = KoiAppearance {
        body_length_pixels,
        identity_seed: koi_development.identity_seed,
        coat_pattern,
    };
    let mut rgba_pixels = Vec::with_capacity((FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize);
    for pixel_y in 0..FRAME_HEIGHT_PIXELS {
        for pixel_x in 0..FRAME_WIDTH_PIXELS {
            let sample_position = [
                f64::from(pixel_x) + 0.5 - f64::from(FRAME_WIDTH_PIXELS) / 2.0 + pose_center[0],
                f64::from(pixel_y) + 0.5 - f64::from(FRAME_HEIGHT_PIXELS) / 2.0 + pose_center[1],
            ];
            rgba_pixels.push(sample_koi(
                &body_spine,
                &fin_curves,
                &barbel_curves,
                sample_position,
                [pixel_x, pixel_y],
                &koi_appearance,
            ));
        }
    }
    paint_outline(&mut rgba_pixels);
    KoiFrame { rgba_pixels }
}

const OUTLINE_COLOR: [u8; 4] = [24, 20, 37, 255];
const DITHER_RANKS: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
const BODY_WIDTH_FRACTION: f64 = 0.2;

struct KoiAppearance {
    body_length_pixels: f64,
    identity_seed: u64,
    coat_pattern: CoatPattern,
}

// Positions and half widths are pixels; consecutive points form tapered strokes.
#[derive(Clone, Copy)]
struct CurvePoint {
    position: [f64; 2],
    half_width_pixels: f64,
}

struct CurveSample {
    along_fraction: f64,
    lateral_pixels: f64,
    distance_pixels: f64,
    half_width_pixels: f64,
    normal_direction: [f64; 2],
}

// Each pair shares its proportions. Direction offsets in radians describe the
// two fins independently, relative to the spine's backward tangent.
struct FinPair {
    root_along_fraction: f64,
    root_side_fraction: f64,
    length_body_widths: f64,
    half_width_body_widths: f64,
    width_start_fraction: f64,
    taper_exponent: f64,
    direction_offsets: [&'static [f64]; 2],
}

const FIN_PAIRS: [FinPair; 3] = [
    FinPair {
        root_along_fraction: 1.0,
        root_side_fraction: 0.0,
        length_body_widths: 1.7,
        half_width_body_widths: 0.3,
        width_start_fraction: 0.12,
        taper_exponent: 0.7,
        direction_offsets: [
            &[-0.18035, -0.25648, -0.37367, -0.50421, -0.61344, -0.66997],
            &[0.248, 0.30115, 0.29856, 0.24064, 0.14695, 0.05079],
        ],
    },
    FinPair {
        root_along_fraction: 0.22,
        root_side_fraction: 0.8,
        length_body_widths: 1.2,
        half_width_body_widths: 0.24,
        width_start_fraction: 0.08,
        taper_exponent: 0.55,
        direction_offsets: [
            &[0.87, 0.77239, 0.71583, 0.70044],
            &[-0.65221, -0.58002, -0.5356, -0.48311],
        ],
    },
    FinPair {
        root_along_fraction: 0.47,
        root_side_fraction: 0.7,
        length_body_widths: 0.6,
        half_width_body_widths: 0.11,
        width_start_fraction: 0.1,
        taper_exponent: 0.6,
        direction_offsets: [&[0.39139, 0.3769, 0.39376], &[-0.62997, -0.48164, -0.32809]],
    },
];

fn build_spine(body_length_pixels: f64) -> Vec<CurvePoint> {
    const HEADING_RADIANS: f64 = -0.08;
    const BEND_RADIANS: f64 = 0.35;
    const WAVE_AMPLITUDE_FRACTION: f64 = 0.06;
    const WAVE_PHASE_RADIANS: f64 = 0.8;
    const WAVE_CYCLES: f64 = 0.9;
    let point_count = body_length_pixels.round() as usize;
    const MIN_HALF_WIDTH_PIXELS: f64 = 1.3;
    let mut center_points = vec![CurvePoint {
        position: [0.0, 0.0],
        half_width_pixels: calculate_body_half_width(0.0, body_length_pixels)
            .max(MIN_HALF_WIDTH_PIXELS),
    }];
    for point_index in 1..point_count {
        let backward_angle_radians =
            HEADING_RADIANS + PI + BEND_RADIANS * point_index as f64 / point_count as f64;
        let [previous_x, previous_y] = center_points[point_index - 1].position;
        center_points.push(CurvePoint {
            position: [
                previous_x + backward_angle_radians.cos(),
                previous_y + backward_angle_radians.sin(),
            ],
            half_width_pixels: calculate_body_half_width(
                point_index as f64 / point_count as f64,
                body_length_pixels,
            )
            .max(MIN_HALF_WIDTH_PIXELS),
        });
    }
    let mut body_spine = Vec::with_capacity(point_count);
    for (point_index, center_point) in center_points.iter().enumerate() {
        let along_fraction = point_index as f64 / point_count as f64;
        let forward_direction = calculate_forward_direction(&center_points, point_index);
        let head_wave_fraction = 0.1;
        let wave_envelope =
            head_wave_fraction + (1.0 - head_wave_fraction) * along_fraction.powi(2);
        let wave_offset = body_length_pixels
            * WAVE_AMPLITUDE_FRACTION
            * wave_envelope
            * (along_fraction * TAU * WAVE_CYCLES - WAVE_PHASE_RADIANS).sin();
        body_spine.push(CurvePoint {
            position: [
                center_point.position[0] - forward_direction[1] * wave_offset,
                center_point.position[1] + forward_direction[0] * wave_offset,
            ],
            half_width_pixels: center_point.half_width_pixels,
        });
    }
    body_spine
}

fn build_fins(body_spine: &[CurvePoint], body_length_pixels: f64) -> Vec<Vec<CurvePoint>> {
    let body_width_pixels = body_length_pixels * BODY_WIDTH_FRACTION;
    let mut fin_curves = Vec::new();
    for fin_pair in &FIN_PAIRS {
        let root_index =
            (fin_pair.root_along_fraction * (body_spine.len() - 1) as f64).round() as usize;
        let root_point = body_spine[root_index];
        let forward_direction = calculate_forward_direction(body_spine, root_index);
        let backward_angle_radians = (-forward_direction[1]).atan2(-forward_direction[0]);
        for (body_side, direction_offsets) in
            [-1.0, 1.0].into_iter().zip(fin_pair.direction_offsets)
        {
            let root_offset_pixels =
                root_point.half_width_pixels * fin_pair.root_side_fraction * body_side;
            let root_position = [
                root_point.position[0] - forward_direction[1] * root_offset_pixels,
                root_point.position[1] + forward_direction[0] * root_offset_pixels,
            ];
            fin_curves.push(build_fin(
                fin_pair,
                root_position,
                backward_angle_radians,
                body_width_pixels,
                direction_offsets,
            ));
        }
    }
    fin_curves
}

fn build_fin(
    fin_pair: &FinPair,
    root_position: [f64; 2],
    backward_angle_radians: f64,
    body_width_pixels: f64,
    direction_offsets: &[f64],
) -> Vec<CurvePoint> {
    let fin_length_pixels = body_width_pixels * fin_pair.length_body_widths;
    let segment_count = fin_length_pixels.ceil() as usize;
    let segment_length_pixels = fin_length_pixels / segment_count as f64;
    let mut center_position = root_position;
    let mut fin_points = Vec::new();
    for point_index in 0..=segment_count {
        let along_fraction = point_index as f64 / segment_count as f64;
        let width_progress =
            fin_pair.width_start_fraction + (1.0 - fin_pair.width_start_fraction) * along_fraction;
        fin_points.push(CurvePoint {
            position: center_position,
            half_width_pixels: body_width_pixels
                * fin_pair.half_width_body_widths
                * (PI * width_progress)
                    .sin()
                    .max(0.0)
                    .powf(fin_pair.taper_exponent),
        });
        let direction_position = (along_fraction * direction_offsets.len() as f64 - 0.5)
            .clamp(0.0, (direction_offsets.len() - 1) as f64);
        let direction_index = direction_position.floor() as usize;
        let start_angle = direction_offsets[direction_index];
        let end_angle = direction_offsets[(direction_index + 1).min(direction_offsets.len() - 1)];
        let segment_angle = backward_angle_radians
            + start_angle
            + (end_angle - start_angle) * direction_position.fract();
        center_position[0] += segment_length_pixels * segment_angle.cos();
        center_position[1] += segment_length_pixels * segment_angle.sin();
    }
    fin_points
}

fn build_barbels(body_spine: &[CurvePoint]) -> Vec<Vec<CurvePoint>> {
    const BARBEL_LENGTH_PIXELS: f64 = 2.5;
    const BARBEL_RADIUS_PIXELS: f64 = 0.35;
    let head_position = body_spine[0].position;
    let forward_direction = calculate_forward_direction(body_spine, 0);
    let heading_radians = forward_direction[1].atan2(forward_direction[0]);
    [-1.0, 1.0]
        .into_iter()
        .map(|body_side| {
            let barbel_root_side_fraction = 0.65;
            let root_offset_pixels =
                body_spine[0].half_width_pixels * barbel_root_side_fraction * body_side;
            let root_position = [
                head_position[0] - heading_radians.sin() * root_offset_pixels,
                head_position[1] + heading_radians.cos() * root_offset_pixels,
            ];
            let barbel_spread_radians = 0.8;
            let barbel_angle = heading_radians + body_side * barbel_spread_radians;
            vec![
                CurvePoint {
                    position: root_position,
                    half_width_pixels: BARBEL_RADIUS_PIXELS,
                },
                CurvePoint {
                    position: [
                        root_position[0] + barbel_angle.cos() * BARBEL_LENGTH_PIXELS,
                        root_position[1] + barbel_angle.sin() * BARBEL_LENGTH_PIXELS,
                    ],
                    half_width_pixels: BARBEL_RADIUS_PIXELS,
                },
            ]
        })
        .collect()
}

fn calculate_pose_center(
    body_spine: &[CurvePoint],
    fin_curves: &[Vec<CurvePoint>],
    barbel_curves: &[Vec<CurvePoint>],
) -> [f64; 2] {
    let mut min_corner = [f64::INFINITY; 2];
    let mut max_corner = [f64::NEG_INFINITY; 2];
    for curve_point in body_spine
        .iter()
        .chain(fin_curves.iter().flatten())
        .chain(barbel_curves.iter().flatten())
    {
        for axis_index in 0..2 {
            min_corner[axis_index] = min_corner[axis_index]
                .min(curve_point.position[axis_index] - curve_point.half_width_pixels);
            max_corner[axis_index] = max_corner[axis_index]
                .max(curve_point.position[axis_index] + curve_point.half_width_pixels);
        }
    }
    [
        (min_corner[0] + max_corner[0]) / 2.0,
        (min_corner[1] + max_corner[1]) / 2.0,
    ]
}

fn sample_koi(
    body_spine: &[CurvePoint],
    fin_curves: &[Vec<CurvePoint>],
    barbel_curves: &[Vec<CurvePoint>],
    sample_position: [f64; 2],
    pixel_position: [u32; 2],
    koi_appearance: &KoiAppearance,
) -> [u8; 4] {
    if let Some(body_sample) = sample_curve(body_spine, sample_position) {
        return shade_body(&body_sample, pixel_position, koi_appearance);
    }
    for fin_curve in fin_curves {
        if let Some(fin_sample) = sample_curve(fin_curve, sample_position) {
            return shade_fin(&fin_sample, pixel_position, koi_appearance.coat_pattern);
        }
    }
    if barbel_curves
        .iter()
        .any(|barbel_curve| sample_curve(barbel_curve, sample_position).is_some())
    {
        return WHITE_RAMP[1];
    }
    [0; 4]
}

// Constructed curves contain at least two points and no zero-length segments.
fn sample_curve(curve_points: &[CurvePoint], sample_position: [f64; 2]) -> Option<CurveSample> {
    let mut closest_sample = None;
    let mut closest_distance_pixels = f64::INFINITY;
    for (segment_index, end_points) in curve_points.windows(2).enumerate() {
        let [start_point, end_point] = [end_points[0], end_points[1]];
        let segment_x = end_point.position[0] - start_point.position[0];
        let segment_y = end_point.position[1] - start_point.position[1];
        let offset_x = sample_position[0] - start_point.position[0];
        let offset_y = sample_position[1] - start_point.position[1];
        let segment_length_pixels = segment_x.hypot(segment_y);
        let segment_progress = ((offset_x * segment_x + offset_y * segment_y)
            / segment_length_pixels.powi(2))
        .clamp(0.0, 1.0);
        let distance_pixels = (offset_x - segment_progress * segment_x)
            .hypot(offset_y - segment_progress * segment_y);
        if distance_pixels >= closest_distance_pixels {
            continue;
        }
        closest_distance_pixels = distance_pixels;
        closest_sample = Some(CurveSample {
            along_fraction: (segment_index as f64 + segment_progress)
                / (curve_points.len() - 1) as f64,
            lateral_pixels: (offset_x * segment_y - offset_y * segment_x) / segment_length_pixels,
            distance_pixels,
            half_width_pixels: start_point.half_width_pixels
                + segment_progress * (end_point.half_width_pixels - start_point.half_width_pixels),
            normal_direction: [
                segment_y / segment_length_pixels,
                -segment_x / segment_length_pixels,
            ],
        });
    }
    closest_sample
        .filter(|curve_sample| curve_sample.distance_pixels <= curve_sample.half_width_pixels)
}

fn shade_body(
    body_sample: &CurveSample,
    pixel_position: [u32; 2],
    koi_appearance: &KoiAppearance,
) -> [u8; 4] {
    let body_length_pixels = koi_appearance.body_length_pixels;
    let lateral_fraction = body_sample.lateral_pixels / body_sample.half_width_pixels;
    let shade_ramp = select_body_ramp(
        koi_appearance.coat_pattern,
        koi_appearance.identity_seed,
        body_sample.along_fraction,
        lateral_fraction,
        body_length_pixels,
        body_sample.half_width_pixels,
    );
    const INNER_RIM_PIXELS: f64 = 0.8;
    if body_sample.distance_pixels > body_sample.half_width_pixels - INNER_RIM_PIXELS {
        return shade_ramp[0];
    }
    let eye_along_fraction = 0.06;
    let eye_lateral_fraction = 0.55;
    let eye_half_thickness_pixels = 0.8;
    let eye_half_span_fraction = 0.25;
    if ((body_sample.along_fraction - eye_along_fraction) * body_length_pixels).abs()
        < eye_half_thickness_pixels
        && (lateral_fraction.abs() - eye_lateral_fraction).abs() < eye_half_span_fraction
    {
        return OUTLINE_COLOR;
    }
    choose_shade(
        shade_ramp,
        calculate_body_brightness(body_sample, lateral_fraction, body_length_pixels),
        pixel_position,
    )
}

fn calculate_body_brightness(
    body_sample: &CurveSample,
    lateral_fraction: f64,
    body_length_pixels: f64,
) -> f64 {
    const AMBIENT_BRIGHTNESS: f64 = 0.24;
    const DIFFUSE_STRENGTH: f64 = 0.7;
    const LIGHT_DIRECTION: [f64; 3] = [-0.32, -0.5, 0.8];
    let surface_normal_z = (1.0 - lateral_fraction.powi(2)).max(0.0).sqrt();
    let incident_light = lateral_fraction
        * (body_sample.normal_direction[0] * LIGHT_DIRECTION[0]
            + body_sample.normal_direction[1] * LIGHT_DIRECTION[1])
        + surface_normal_z * LIGHT_DIRECTION[2];
    let mut surface_brightness = AMBIENT_BRIGHTNESS + DIFFUSE_STRENGTH * incident_light.max(0.0);
    let dorsal_span = 0.27..0.62;
    let dorsal_half_width_fraction = 0.1;
    let dorsal_shadow = 0.18;
    if dorsal_span.contains(&body_sample.along_fraction)
        && lateral_fraction.abs() < dorsal_half_width_fraction
    {
        surface_brightness -= dorsal_shadow;
    }
    let gill_along_fraction = 0.135;
    let gill_inner_fraction = 0.3;
    let gill_shadow = 0.22;
    if ((body_sample.along_fraction - gill_along_fraction) * body_length_pixels).abs() < 0.5
        && lateral_fraction.abs() > gill_inner_fraction
    {
        surface_brightness -= gill_shadow;
    }
    surface_brightness
}

fn shade_fin(
    fin_sample: &CurveSample,
    pixel_position: [u32; 2],
    coat_pattern: CoatPattern,
) -> [u8; 4] {
    const FIN_RIM_PIXELS: f64 = 0.6;
    const FIN_RAY_SPACING_PIXELS: f64 = 1.8;
    const FIN_ROOT_SHADOW: f64 = 0.12;
    let shade_ramp = select_fin_ramp(
        coat_pattern,
        fin_sample.along_fraction,
        fin_sample.lateral_pixels / fin_sample.half_width_pixels,
    );
    if fin_sample.distance_pixels > fin_sample.half_width_pixels - FIN_RIM_PIXELS {
        return shade_ramp[1];
    }
    let ray_phase = (fin_sample.lateral_pixels / FIN_RAY_SPACING_PIXELS).rem_euclid(1.0);
    let ray_brightness = 0.68;
    let membrane_brightness = 0.94;
    let surface_brightness = if ray_phase < 0.5 {
        ray_brightness
    } else {
        membrane_brightness
    };
    let root_shadow = FIN_ROOT_SHADOW * (1.0 - fin_sample.along_fraction).powi(2);
    choose_shade(shade_ramp, surface_brightness - root_shadow, pixel_position)
}

fn calculate_body_half_width(along_fraction: f64, body_length_pixels: f64) -> f64 {
    const HEAD_LENGTH_FRACTION: f64 = 0.12;
    const SHOULDER_FRACTION: f64 = 0.3;
    const TAPER_LENGTH_FRACTION: f64 = 0.62;
    const TAPER_EXPONENT: f64 = 1.8;
    const TAIL_WIDTH_FRACTION: f64 = 0.26;
    let width_fraction = if along_fraction < HEAD_LENGTH_FRACTION {
        let nose_width_fraction = 0.55;
        nose_width_fraction
            + (1.0 - nose_width_fraction) * (along_fraction / HEAD_LENGTH_FRACTION * PI / 2.0).sin()
    } else {
        (1.0 - ((along_fraction - SHOULDER_FRACTION).max(0.0) / TAPER_LENGTH_FRACTION)
            .powf(TAPER_EXPONENT))
        .max(TAIL_WIDTH_FRACTION)
    };
    body_length_pixels * BODY_WIDTH_FRACTION / 2.0 * width_fraction
}

fn calculate_forward_direction(curve_points: &[CurvePoint], point_index: usize) -> [f64; 2] {
    const DIRECTION_SPAN_SAMPLES: usize = 2;
    let earlier_point = curve_points[point_index.saturating_sub(DIRECTION_SPAN_SAMPLES)].position;
    let later_point =
        curve_points[(point_index + DIRECTION_SPAN_SAMPLES).min(curve_points.len() - 1)].position;
    let tangent_x = earlier_point[0] - later_point[0];
    let tangent_y = earlier_point[1] - later_point[1];
    let tangent_length_pixels = tangent_x.hypot(tangent_y);
    [
        tangent_x / tangent_length_pixels,
        tangent_y / tangent_length_pixels,
    ]
}

fn paint_outline(rgba_pixels: &mut [[u8; 4]]) {
    let original_pixels = rgba_pixels.to_vec();
    let frame_width = FRAME_WIDTH_PIXELS as usize;
    for (pixel_index, pixel_channels) in rgba_pixels.iter_mut().enumerate() {
        if original_pixels[pixel_index][3] != 0 {
            continue;
        }
        let pixel_x = pixel_index % frame_width;
        let pixel_y = pixel_index / frame_width;
        let touches_shape = (pixel_x > 0 && original_pixels[pixel_index - 1][3] != 0)
            || (pixel_x + 1 < frame_width && original_pixels[pixel_index + 1][3] != 0)
            || (pixel_y > 0 && original_pixels[pixel_index - frame_width][3] != 0)
            || (pixel_y + 1 < FRAME_HEIGHT_PIXELS as usize
                && original_pixels[pixel_index + frame_width][3] != 0);
        if touches_shape {
            *pixel_channels = OUTLINE_COLOR;
        }
    }
}

fn choose_shade(
    shade_ramp: &[[u8; 4]],
    surface_brightness: f64,
    pixel_position: [u32; 2],
) -> [u8; 4] {
    let shade_position = surface_brightness.clamp(0.0, 1.0) * (shade_ramp.len() - 1) as f64;
    let base_index = shade_position.floor() as usize;
    let dither_x = pixel_position[0] as usize % DITHER_RANKS[0].len();
    let dither_y = pixel_position[1] as usize % DITHER_RANKS.len();
    let dither_threshold = (f64::from(DITHER_RANKS[dither_y][dither_x]) + 0.5)
        / (DITHER_RANKS.len() * DITHER_RANKS[0].len()) as f64;
    let shade_index = base_index + usize::from(shade_position.fract() > dither_threshold);
    shade_ramp[shade_index.min(shade_ramp.len() - 1)]
}
