//! Classic koi coats sampled in body coordinates, independent of account policy.

/// Supported coats. Callers select the design; language and streak mapping is separate.
#[derive(Clone, Copy, Debug)]
pub enum CoatPattern {
    Plain,
    Kohaku,
    Sanke,
    Showa,
    Tancho,
    Asagi,
}

pub(crate) const WHITE_RAMP: [[u8; 4]; 4] = [
    [90, 105, 136, 255],
    [139, 155, 180, 255],
    [192, 203, 220, 255],
    [255, 255, 255, 255],
];
const RED_RAMP: [[u8; 4]; 4] = [
    [162, 38, 51, 255],
    [228, 59, 68, 255],
    [240, 96, 79, 255],
    [246, 117, 122, 255],
];
const BLACK_RAMP: [[u8; 4]; 3] = [[24, 20, 37, 255], [38, 43, 68, 255], [58, 68, 102, 255]];
const BLUE_RAMP: [[u8; 4]; 4] = [
    [38, 43, 68, 255],
    [58, 68, 102, 255],
    [90, 105, 136, 255],
    [139, 155, 180, 255],
];
const ORANGE_RAMP: [[u8; 4]; 3] = [[190, 74, 47, 255], [247, 118, 34, 255], [254, 174, 52, 255]];
const SCALE_EDGE_RAMP: [[u8; 4]; 1] = [[58, 68, 102, 255]];

pub(crate) fn select_body_ramp(
    coat_pattern: CoatPattern,
    identity_seed: u64,
    along_fraction: f64,
    lateral_fraction: f64,
    body_length_pixels: f64,
    half_width_pixels: f64,
) -> &'static [[u8; 4]] {
    match coat_pattern {
        CoatPattern::Plain => &WHITE_RAMP,
        CoatPattern::Tancho => {
            if ((along_fraction - 0.09) / 0.065).hypot(lateral_fraction / 0.55) < 1.0 {
                &RED_RAMP
            } else {
                &WHITE_RAMP
            }
        }
        CoatPattern::Asagi => select_asagi_ramp(
            along_fraction,
            lateral_fraction,
            body_length_pixels,
            half_width_pixels,
        ),
        CoatPattern::Kohaku | CoatPattern::Sanke | CoatPattern::Showa => select_patch_ramp(
            coat_pattern,
            identity_seed,
            along_fraction,
            lateral_fraction,
        ),
    }
}

pub(crate) fn select_fin_ramp(
    coat_pattern: CoatPattern,
    along_fraction: f64,
    lateral_fraction: f64,
) -> &'static [[u8; 4]] {
    match coat_pattern {
        CoatPattern::Asagi => &ORANGE_RAMP,
        CoatPattern::Showa if along_fraction < 0.28 && lateral_fraction.abs() < 0.8 => &BLACK_RAMP,
        _ => &WHITE_RAMP,
    }
}

fn select_patch_ramp(
    coat_pattern: CoatPattern,
    identity_seed: u64,
    along_fraction: f64,
    lateral_fraction: f64,
) -> &'static [[u8; 4]] {
    let patch_noise = sample_fractal_noise(
        identity_seed,
        along_fraction * 9.0,
        lateral_fraction * 2.0 + 5.0,
    );
    let edge_offset = (patch_noise - 0.5) * 0.3;
    let red_plates = [(0.03, 0.19), (0.31, 0.46), (0.58, 0.74)];
    let has_red_patch = lateral_fraction.abs() < 0.7 + edge_offset
        && red_plates.iter().any(|&(plate_start, plate_end)| {
            along_fraction >= plate_start - edge_offset * 0.08
                && along_fraction <= plate_end + edge_offset * 0.08
        });
    let mut shade_ramp: &[[u8; 4]] = &WHITE_RAMP;
    if matches!(coat_pattern, CoatPattern::Showa)
        && sample_fractal_noise(
            identity_seed,
            along_fraction * 5.0 + 120.0,
            lateral_fraction * 1.5,
        ) <= 0.58
    {
        shade_ramp = &BLACK_RAMP;
    }
    if has_red_patch {
        shade_ramp = &RED_RAMP;
    }
    let has_black_patch = match coat_pattern {
        CoatPattern::Sanke => {
            (0.14..0.8).contains(&along_fraction)
                && lateral_fraction.abs() < 0.75
                && sample_fractal_noise(
                    identity_seed,
                    along_fraction * 10.0 + 30.0,
                    lateral_fraction * 3.0,
                ) > 0.68
        }
        CoatPattern::Showa => {
            (along_fraction < 0.13
                && (lateral_fraction - 0.25 * (along_fraction * 55.0).sin()).abs() < 0.14)
                || sample_fractal_noise(
                    identity_seed,
                    along_fraction * 4.0 + 60.0,
                    lateral_fraction * 1.2,
                ) > 0.47
        }
        _ => false,
    };
    if has_black_patch {
        &BLACK_RAMP
    } else {
        shade_ramp
    }
}

fn select_asagi_ramp(
    along_fraction: f64,
    lateral_fraction: f64,
    body_length_pixels: f64,
    half_width_pixels: f64,
) -> &'static [[u8; 4]] {
    if along_fraction < 0.11 {
        return &WHITE_RAMP;
    }
    if lateral_fraction.abs() > 0.64 {
        return &ORANGE_RAMP;
    }
    let scale_row = (lateral_fraction + 1.0) * half_width_pixels / 2.2;
    let scale_column =
        along_fraction * body_length_pixels / 2.6 + scale_row.floor().rem_euclid(2.0) * 0.5;
    if scale_row.fract().min(scale_column.fract()) < 0.24 {
        &SCALE_EDGE_RAMP
    } else {
        &BLUE_RAMP
    }
}

fn sample_fractal_noise(identity_seed: u64, coordinate_x: f64, coordinate_y: f64) -> f64 {
    let mut noise_sum = 0.0;
    let mut total_weight = 0.0;
    for octave_index in 0..3 {
        let frequency = 2.0_f64.powi(octave_index);
        let noise_weight = 1.0 / frequency;
        noise_sum += sample_smooth_noise(
            identity_seed,
            coordinate_x * frequency,
            coordinate_y * frequency,
        ) * noise_weight;
        total_weight += noise_weight;
    }
    noise_sum / total_weight
}

fn sample_smooth_noise(identity_seed: u64, coordinate_x: f64, coordinate_y: f64) -> f64 {
    let lattice_x = coordinate_x.floor() as i64;
    let lattice_y = coordinate_y.floor() as i64;
    let fraction_x = coordinate_x - coordinate_x.floor();
    let fraction_y = coordinate_y - coordinate_y.floor();
    let blend_x = fraction_x * fraction_x * (3.0 - 2.0 * fraction_x);
    let blend_y = fraction_y * fraction_y * (3.0 - 2.0 * fraction_y);
    let top_left = hash_lattice_point(identity_seed, lattice_x, lattice_y);
    let top_right = hash_lattice_point(identity_seed, lattice_x + 1, lattice_y);
    let bottom_left = hash_lattice_point(identity_seed, lattice_x, lattice_y + 1);
    let bottom_right = hash_lattice_point(identity_seed, lattice_x + 1, lattice_y + 1);
    let top_noise = top_left + (top_right - top_left) * blend_x;
    let bottom_noise = bottom_left + (bottom_right - bottom_left) * blend_x;
    top_noise + (bottom_noise - top_noise) * blend_y
}

fn hash_lattice_point(identity_seed: u64, lattice_x: i64, lattice_y: i64) -> f64 {
    let mut hash_bits = identity_seed
        ^ (lattice_x as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (lattice_y as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    hash_bits = (hash_bits ^ (hash_bits >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    hash_bits = (hash_bits ^ (hash_bits >> 27)).wrapping_mul(0x94d049bb133111eb);
    hash_bits ^= hash_bits >> 31;
    (hash_bits >> 11) as f64 / (1_u64 << 53) as f64
}
