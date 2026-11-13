//! The pond a koi swims in: the native canvas's size and the lap a koi swims there.
//!
//! The default pond is the square [`crate::render::FRAME_WIDTH_PIXELS`] by
//! [`crate::render::FRAME_HEIGHT_PIXELS`] canvas. In a square pond, laps circle the
//! center. A wider or taller pond swims a track drawn for the account and the
//! observation date, along the pond's long side: an oval or a figure-eight, swum either
//! way round, with loop-the-loops and S-wiggles that curl either way. Each day draws a
//! new track, and reruns of a day draw the same one. A longer lap lasts a whole number
//! of times as long, so the koi keeps about the same pace in every pond.

use std::error::Error;
use std::f64::consts::{PI, TAU};
use std::fmt;
use std::str::FromStr;

use chrono::Datelike;

use crate::koi::KoiDevelopment;
use crate::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS};

/// The size of the native canvas a koi is drawn on. The default is the square
/// [`FRAME_WIDTH_PIXELS`] by [`FRAME_HEIGHT_PIXELS`] canvas.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PondSize {
    width_pixels: u32,
    height_pixels: u32,
}

impl PondSize {
    /// The shortest side a pond may have: room for the longest koi to turn.
    pub const MIN_SIDE_PIXELS: u32 = 256;
    /// The longest side a pond may have.
    pub const MAX_SIDE_PIXELS: u32 = 4096;

    /// A pond of this many native pixels across and down, each from
    /// [`Self::MIN_SIDE_PIXELS`] through [`Self::MAX_SIDE_PIXELS`].
    pub fn new(width_pixels: u32, height_pixels: u32) -> Result<Self, PondSizeError> {
        let side_range = Self::MIN_SIDE_PIXELS..=Self::MAX_SIDE_PIXELS;
        if side_range.contains(&width_pixels) && side_range.contains(&height_pixels) {
            Ok(Self {
                width_pixels,
                height_pixels,
            })
        } else {
            Err(PondSizeError::OutOfRange {
                width_pixels,
                height_pixels,
            })
        }
    }

    /// Native pixels across.
    pub fn width_pixels(self) -> u32 {
        self.width_pixels
    }

    /// Native pixels down.
    pub fn height_pixels(self) -> u32 {
        self.height_pixels
    }

    pub(crate) fn pixel_count(self) -> usize {
        self.width_pixels as usize * self.height_pixels as usize
    }

    fn short_side_pixels(self) -> u32 {
        self.width_pixels.min(self.height_pixels)
    }
}

impl Default for PondSize {
    fn default() -> Self {
        Self {
            width_pixels: FRAME_WIDTH_PIXELS,
            height_pixels: FRAME_HEIGHT_PIXELS,
        }
    }
}

/// Written as `WIDTHxHEIGHT`, such as `768x256`.
impl fmt::Display for PondSize {
    fn fmt(&self, output_formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output_formatter,
            "{}x{}",
            self.width_pixels, self.height_pixels
        )
    }
}

/// Read as `WIDTHxHEIGHT`, such as `768x256`.
impl FromStr for PondSize {
    type Err = PondSizeError;

    fn from_str(size_text: &str) -> Result<Self, Self::Err> {
        let (width_text, height_text) = size_text
            .split_once('x')
            .ok_or_else(|| PondSizeError::Malformed(size_text.to_owned()))?;
        let parse_side = |side_text: &str| {
            if side_text.is_empty() || !side_text.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(PondSizeError::Malformed(size_text.to_owned()));
            }
            // Too many digits for a u32 is out of range rather than malformed.
            Ok(side_text.parse().unwrap_or(u32::MAX))
        };
        Self::new(parse_side(width_text)?, parse_side(height_text)?)
    }
}

/// Why a pond size was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PondSizeError {
    /// The text is not `WIDTHxHEIGHT` in whole pixels; carries the text.
    Malformed(String),
    /// A side is shorter than [`PondSize::MIN_SIDE_PIXELS`] or longer than
    /// [`PondSize::MAX_SIDE_PIXELS`].
    OutOfRange {
        /// The width asked for.
        width_pixels: u32,
        /// The height asked for.
        height_pixels: u32,
    },
}

impl fmt::Display for PondSizeError {
    fn fmt(&self, output_formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(size_text) => write!(
                output_formatter,
                "pond size {size_text:?} is not WIDTHxHEIGHT in pixels, such as 768x256"
            ),
            Self::OutOfRange {
                width_pixels,
                height_pixels,
            } => write!(
                output_formatter,
                "pond size {width_pixels}x{height_pixels} needs each side from {} through {} pixels",
                PondSize::MIN_SIDE_PIXELS,
                PondSize::MAX_SIDE_PIXELS
            ),
        }
    }
}

impl Error for PondSizeError {}

/// About how far a cruising koi swims in one loop of [`crate::motion::frame_count`]
/// frames; a lap this many times longer lasts that many loops.
const LAP_PIXELS_PER_LOOP: f64 = 600.0;
/// A track is drawn again when its pace strays further than this share from
/// LAP_PIXELS_PER_LOOP per loop, so the koi swims about as fast in every pond.
const PACE_TOLERANCE: f64 = 0.15;

/// The lap a koi swims in a pond, and how many loops it lasts.
pub(crate) struct Lap {
    path: LapPath,
    /// How many times as long as the default loop each lap lasts.
    stretch: u32,
}

enum LapPath {
    Circle { radius_pixels: f64 },
    Track(LapTrack),
}

impl Lap {
    /// The day's lap for a koi of this length: a circle in a square pond, or the day's
    /// track in any other.
    pub(crate) fn for_day(
        koi_development: &KoiDevelopment,
        pond_size: PondSize,
        body_length_pixels: f64,
    ) -> Self {
        let widest_radius = calculate_lap_radius(body_length_pixels, pond_size);
        let path = if pond_size.width_pixels == pond_size.height_pixels {
            LapPath::Circle {
                radius_pixels: widest_radius,
            }
        } else {
            let day_number = koi_development.observed_on.num_days_from_ce() as u64;
            LapPath::Track(LapTrack::generate(
                koi_development.identity_seed ^ day_number.rotate_left(32),
                pond_size,
                widest_radius,
            ))
        };
        let length_pixels = match &path {
            LapPath::Circle { radius_pixels } => TAU * radius_pixels,
            LapPath::Track(lap_track) => lap_track.length_pixels(),
        };
        Self {
            path,
            stretch: count_lap_loops(length_pixels),
        }
    }

    /// How many times as long as the default loop each lap lasts.
    pub(crate) fn stretch(&self) -> u32 {
        self.stretch
    }

    /// Where a point this far behind the head sits, measured along the lap, when the
    /// head is at this lap angle: a full turn per lap, starting at the top.
    pub(crate) fn position_behind(&self, angle_radians: f64, behind_pixels: f64) -> [f64; 2] {
        match &self.path {
            LapPath::Circle { radius_pixels } => {
                let point_angle_radians = angle_radians - behind_pixels / radius_pixels;
                [
                    radius_pixels * point_angle_radians.cos(),
                    radius_pixels * point_angle_radians.sin(),
                ]
            }
            LapPath::Track(lap_track) => {
                let head_distance_pixels =
                    (angle_radians + PI / 2.0).rem_euclid(TAU) / TAU * lap_track.length_pixels();
                lap_track.position_at(head_distance_pixels - behind_pixels)
            }
        }
    }
}

/// How many loops a lap of this length lasts, so the koi keeps about its pace.
fn count_lap_loops(length_pixels: f64) -> u32 {
    (length_pixels / LAP_PIXELS_PER_LOOP).round().max(1.0) as u32
}

/// The widest lap turn that keeps the whole fish inside the pond's short side: tighter
/// for longer koi, and never so tight that the body covers more than its share of the
/// circle. In the default pond this is the circle laps have always swum.
pub(crate) fn calculate_lap_radius(body_length_pixels: f64, pond_size: PondSize) -> f64 {
    // Room past the center line for the fins, outline, whiskers, and a clear edge.
    const EDGE_ROOM_PIXELS: f64 = 12.0;
    const OUTER_REACH_FRACTION: f64 = 0.33;
    // How much closer the widest lap stays than the pond's reach.
    const LAP_INSET_PIXELS: f64 = 28.0;
    const MAX_BODY_SHARE_OF_LAP: f64 = 0.6;
    let pond_reach_pixels = f64::from(pond_size.short_side_pixels()) / 2.0 - EDGE_ROOM_PIXELS;
    (pond_reach_pixels - OUTER_REACH_FRACTION * body_length_pixels)
        .min(pond_reach_pixels - LAP_INSET_PIXELS)
        .max(body_length_pixels / (TAU * MAX_BODY_SHARE_OF_LAP))
}

/// A closed track through points one pixel or less apart.
struct LapTrack {
    /// The points in swimming order, closing back on the first.
    points: Vec<[f64; 2]>,
    /// Distance along the track to each point; the last is the track's length.
    distances: Vec<f64>,
}

/// The tightest turn on any track, before easing: the longest koi covers no more than
/// six tenths of a circle this size, even after easing tightens it slightly.
const MIN_TURN_RADIUS_PIXELS: f64 = 48.0;
/// End turns range from the widest lap that fits down to this much tighter.
const END_RADIUS_SPREAD_PIXELS: f64 = 24.0;
/// How far easing spreads each change of turn along the track.
const EASING_PIXELS: f64 = 10.0;
const TRACK_ATTEMPTS: u32 = 400;

/// One piece of track: a straight of a length, or a turn of a radius through an angle,
/// clockwise on screen when positive.
#[derive(Clone, Copy)]
enum TrackMove {
    Straight(f64),
    Turn(f64, f64),
}

impl LapTrack {
    /// A track along the pond's long side whose widest turn is `widest_radius`, so the
    /// koi keeps the same margin from the long sides as from the short ones. Layouts
    /// that would leave the pond or stray from the koi's pace are drawn again; if none
    /// fits, the track is a plain oval.
    fn generate(seed: u64, pond_size: PondSize, widest_radius: f64) -> Self {
        let mut random = SplitMix64(seed);
        let long_side = pond_size.width_pixels.max(pond_size.height_pixels);
        let half_size = [
            f64::from(long_side - pond_size.short_side_pixels()) / 2.0 + widest_radius,
            widest_radius,
        ];
        let mut chosen_points = None;
        for _ in 0..TRACK_ATTEMPTS {
            let end_radius = random.uniform(
                (widest_radius - END_RADIUS_SPREAD_PIXELS).max(MIN_TURN_RADIUS_PIXELS),
                widest_radius,
            );
            // A figure-eight needs its lobes apart to cross between them.
            let has_room_for_eight = half_size[0] - end_radius > end_radius;
            let (moves, start, heading) = if random.chance(0.5) || !has_room_for_eight {
                draw_oval(&mut random, half_size[0], end_radius)
            } else {
                draw_figure_eight(&mut random, half_size[0], end_radius)
            };
            let mut points = trace(&moves, start, heading);
            // Mirroring top to bottom swims the same layout the other way round.
            if random.chance(0.5) {
                for point in &mut points {
                    point[1] = -point[1];
                }
            }
            let fits = points.iter().all(|point| {
                (0..2).all(|axis_index| point[axis_index].abs() <= half_size[axis_index] + 0.5)
            });
            let eased_points = ease(&points);
            let length_pixels = measure_distances(&eased_points)[eased_points.len() - 1];
            let pace =
                length_pixels / (LAP_PIXELS_PER_LOOP * f64::from(count_lap_loops(length_pixels)));
            if fits && (pace - 1.0).abs() <= PACE_TOLERANCE {
                chosen_points = Some(eased_points);
                break;
            }
        }
        let mut points = chosen_points.unwrap_or_else(|| {
            let (moves, start, heading) = draw_plain_oval(half_size[0], widest_radius);
            ease(&trace(&moves, start, heading))
        });
        // A tall pond swims the same layouts along its height.
        if pond_size.height_pixels > pond_size.width_pixels {
            for point in &mut points {
                point.swap(0, 1);
            }
        }
        Self::from_points(points)
    }

    fn from_points(points: Vec<[f64; 2]>) -> Self {
        let distances = measure_distances(&points);
        Self { points, distances }
    }

    fn length_pixels(&self) -> f64 {
        self.distances[self.distances.len() - 1]
    }

    /// The point this far along the track, wrapping around it either way.
    fn position_at(&self, distance_pixels: f64) -> [f64; 2] {
        let distance = distance_pixels.rem_euclid(self.length_pixels());
        let next_index = self
            .distances
            .partition_point(|&point_distance| point_distance <= distance)
            .clamp(1, self.points.len() - 1);
        let [from, to] = [self.points[next_index - 1], self.points[next_index]];
        let [from_distance, to_distance] =
            [self.distances[next_index - 1], self.distances[next_index]];
        let fraction = (distance - from_distance) / (to_distance - from_distance).max(f64::EPSILON);
        [0, 1].map(|axis_index| from[axis_index] + (to[axis_index] - from[axis_index]) * fraction)
    }
}

/// Distance along the points to each one, from zero at the first.
fn measure_distances(points: &[[f64; 2]]) -> Vec<f64> {
    let mut distances = Vec::with_capacity(points.len());
    distances.push(0.0);
    for pair in points.windows(2) {
        let step = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
        distances.push(distances[distances.len() - 1] + step);
    }
    distances
}

/// The moves of a track, where it starts, and its heading there.
type TrackLayout = (Vec<TrackMove>, [f64; 2], f64);

/// An oval with no loops or wiggles, swum clockwise from the top left.
fn draw_plain_oval(half_width: f64, end_radius: f64) -> TrackLayout {
    let straight = 2.0 * (half_width - end_radius);
    (
        vec![
            TrackMove::Straight(straight),
            TrackMove::Turn(end_radius, PI),
            TrackMove::Straight(straight),
            TrackMove::Turn(end_radius, PI),
        ],
        [-straight / 2.0, -end_radius],
        0.0,
    )
}

/// An oval swum clockwise from the top left: two straights joined by half turns,
/// with features along both straights.
fn draw_oval(random: &mut SplitMix64, half_width: f64, end_radius: f64) -> TrackLayout {
    let straight = 2.0 * (half_width - end_radius);
    let mut moves = place_features(random, straight);
    moves.push(TrackMove::Turn(end_radius, PI));
    moves.extend(place_features(random, straight));
    moves.push(TrackMove::Turn(end_radius, PI));
    (moves, [-straight / 2.0, -end_radius], 0.0)
}

/// A figure-eight from the center, heading down and right: counterclockwise round the
/// right lobe and clockwise round the left, crossing in the middle, with features
/// along each half of both crossing lines.
fn draw_figure_eight(random: &mut SplitMix64, half_width: f64, end_radius: f64) -> TrackLayout {
    let lobe_offset = half_width - end_radius;
    let crossing_angle = (end_radius / lobe_offset).asin();
    let half_crossing = (lobe_offset * lobe_offset - end_radius * end_radius).sqrt();
    let lobe_sweep = PI + 2.0 * crossing_angle;
    let mut moves = place_features(random, half_crossing);
    moves.push(TrackMove::Turn(end_radius, -lobe_sweep));
    moves.extend(place_features(random, half_crossing));
    moves.extend(place_features(random, half_crossing));
    moves.push(TrackMove::Turn(end_radius, lobe_sweep));
    moves.extend(place_features(random, half_crossing));
    (moves, [0.0, 0.0], crossing_angle)
}

/// Loop-the-loops or S-wiggles along a straight, up to two or one per FEATURE_SPACING
/// pixels, whichever is more, each curling either way and spaced so none overlap. A
/// loop circles back to where it began and a wiggle returns to the straight's line, so
/// features never change where the track goes.
fn place_features(random: &mut SplitMix64, straight: f64) -> Vec<TrackMove> {
    const FEATURE_SPACING_PIXELS: f64 = 300.0;
    const LOOP_SHARE: f64 = 0.55;
    const MAX_LOOP_RADIUS_PIXELS: f64 = 64.0;
    const MAX_WIGGLE_RADIUS_PIXELS: f64 = 80.0;
    const WIGGLE_ANGLE_RANGE_RADIANS: (f64, f64) = (0.5, 1.05);
    const LOOP_CLEARANCE_PIXELS: f64 = 16.0;
    const WIGGLE_CLEARANCE_PIXELS: f64 = 8.0;
    // Room left clear at each end of the straight, so features don't crowd turns.
    const END_CLEARANCE_PIXELS: f64 = 12.0;
    let max_feature_count = ((straight / FEATURE_SPACING_PIXELS) as u64).max(2);
    let feature_count = (random.next() % (max_feature_count + 1)) as usize;
    if feature_count == 0 {
        return vec![TrackMove::Straight(straight)];
    }
    // Each feature: which way it curls, its radius, and a wiggle's angle.
    let features: Vec<(f64, f64, Option<f64>)> = (0..feature_count)
        .map(|_| {
            let spin = if random.chance(0.5) { 1.0 } else { -1.0 };
            if random.chance(LOOP_SHARE) {
                let radius = random.uniform(MIN_TURN_RADIUS_PIXELS, MAX_LOOP_RADIUS_PIXELS);
                (spin, radius, None)
            } else {
                let radius = random.uniform(MIN_TURN_RADIUS_PIXELS, MAX_WIGGLE_RADIUS_PIXELS);
                let angle =
                    random.uniform(WIGGLE_ANGLE_RANGE_RADIANS.0, WIGGLE_ANGLE_RANGE_RADIANS.1);
                (spin, radius, Some(angle))
            }
        })
        .collect();
    // How far along the straight each feature carries the koi, and how much of the
    // straight it covers.
    let advances: Vec<f64> = features
        .iter()
        .map(|&(_, radius, wiggle)| wiggle.map_or(0.0, |angle| 4.0 * radius * angle.sin()))
        .collect();
    let reaches: Vec<f64> = features
        .iter()
        .zip(&advances)
        .map(|(&(_, radius, wiggle), advance)| match wiggle {
            None => 2.0 * radius + LOOP_CLEARANCE_PIXELS,
            Some(_) => advance + WIGGLE_CLEARANCE_PIXELS,
        })
        .collect();
    let free = straight - advances.iter().sum::<f64>();
    let weights: Vec<f64> = (0..=feature_count)
        .map(|_| random.uniform(0.3, 1.3))
        .collect();
    let weight_total: f64 = weights.iter().sum();
    let gaps: Vec<f64> = weights
        .iter()
        .map(|weight| free * weight / weight_total)
        .collect();
    let mut position = 0.0;
    let mut spans = Vec::with_capacity(feature_count);
    for feature_index in 0..feature_count {
        position += gaps[feature_index];
        let middle = position + advances[feature_index] / 2.0;
        spans.push((
            middle - reaches[feature_index] / 2.0,
            middle + reaches[feature_index] / 2.0,
        ));
        position += advances[feature_index];
    }
    let fits = free > 0.0
        && spans[0].0 > END_CLEARANCE_PIXELS
        && spans[feature_count - 1].1 < straight - END_CLEARANCE_PIXELS
        && spans.windows(2).all(|pair| pair[0].1 < pair[1].0);
    if !fits {
        return vec![TrackMove::Straight(straight)];
    }
    let mut moves = Vec::new();
    for (feature_index, &(spin, radius, wiggle)) in features.iter().enumerate() {
        moves.push(TrackMove::Straight(gaps[feature_index]));
        match wiggle {
            None => moves.push(TrackMove::Turn(radius, spin * TAU)),
            Some(angle) => moves.extend([
                TrackMove::Turn(radius, spin * angle),
                TrackMove::Turn(radius, -2.0 * spin * angle),
                TrackMove::Turn(radius, spin * angle),
            ]),
        }
    }
    moves.push(TrackMove::Straight(gaps[feature_count]));
    moves
}

/// Follow the moves from a start point and heading, one pixel or less per point.
fn trace(moves: &[TrackMove], start: [f64; 2], start_heading: f64) -> Vec<[f64; 2]> {
    let mut points = vec![start];
    let mut heading = start_heading;
    for &track_move in moves {
        let [x, y] = points[points.len() - 1];
        match track_move {
            TrackMove::Straight(length) => {
                let step_count = length.ceil().max(1.0) as usize;
                points.extend((1..=step_count).map(|step_index| {
                    let along = length * step_index as f64 / step_count as f64;
                    [x + along * heading.cos(), y + along * heading.sin()]
                }));
            }
            TrackMove::Turn(radius, sweep) => {
                // The center sits on the side the koi turns toward.
                let side = sweep.signum();
                let center = [
                    x - side * radius * heading.sin(),
                    y + side * radius * heading.cos(),
                ];
                let start_angle = heading - side * PI / 2.0;
                let step_count = (sweep.abs() * radius).ceil().max(1.0) as usize;
                points.extend((1..=step_count).map(|step_index| {
                    let angle = start_angle + sweep * step_index as f64 / step_count as f64;
                    [
                        center[0] + radius * angle.cos(),
                        center[1] + radius * angle.sin(),
                    ]
                }));
                heading += sweep;
            }
        }
    }
    points
}

/// Respace a closed track's points one pixel apart, then blur them along the track,
/// so each change of turn spreads over EASING_PIXELS instead of snapping. A closed
/// track stays closed, and its turns tighten only slightly.
fn ease(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let raw = LapTrack::from_points(points.to_vec());
    let point_count = raw.length_pixels().round().max(3.0) as usize;
    let step = raw.length_pixels() / point_count as f64;
    let even: Vec<[f64; 2]> = (0..point_count)
        .map(|point_index| raw.position_at(point_index as f64 * step))
        .collect();
    let reach = (3.0 * EASING_PIXELS / step).ceil() as isize;
    let weights: Vec<f64> = (-reach..=reach)
        .map(|offset| (-0.5 * (offset as f64 * step / EASING_PIXELS).powi(2)).exp())
        .collect();
    let weight_total: f64 = weights.iter().sum();
    let mut eased: Vec<[f64; 2]> = (0..point_count as isize)
        .map(|point_index| {
            let mut sum = [0.0; 2];
            for (weight, offset) in weights.iter().zip(-reach..=reach) {
                let neighbor =
                    even[(point_index + offset).rem_euclid(point_count as isize) as usize];
                sum[0] += weight * neighbor[0];
                sum[1] += weight * neighbor[1];
            }
            sum.map(|total| total / weight_total)
        })
        .collect();
    eased.push(eased[0]);
    eased
}

/// A small, fixed random sequence, so a seed always draws the same track.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn chance(&mut self, probability: f64) -> bool {
        self.uniform(0.0, 1.0) < probability
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_pond_keeps_the_original_lap_circle() {
        for body_length_pixels in [20.0, 60.0, 90.0, 130.0] {
            let original_radius = (116.0_f64 - 0.33 * body_length_pixels)
                .min(88.0)
                .max(body_length_pixels / (TAU * 0.6));
            assert_eq!(
                calculate_lap_radius(body_length_pixels, PondSize::default()),
                original_radius
            );
            assert_eq!(count_lap_loops(TAU * original_radius), 1);
        }
    }

    #[test]
    fn tracks_close_stay_inside_and_keep_the_pace() {
        for (pond_size, body_length_pixels) in [
            (PondSize::new(768, 256).unwrap(), 60.0),
            (PondSize::new(768, 256).unwrap(), 130.0),
            (PondSize::new(300, 256).unwrap(), 90.0),
            (PondSize::new(256, 900).unwrap(), 90.0),
            (PondSize::new(2000, 600).unwrap(), 40.0),
        ] {
            let widest_radius = calculate_lap_radius(body_length_pixels, pond_size);
            let half_size =
                [pond_size.width_pixels, pond_size.height_pixels].map(|side| f64::from(side) / 2.0);
            let margin = f64::from(pond_size.short_side_pixels()) / 2.0 - widest_radius;
            for seed in 0..40 {
                let lap_track = LapTrack::generate(seed, pond_size, widest_radius);
                let first = lap_track.points[0];
                let last = lap_track.points[lap_track.points.len() - 1];
                assert!((first[0] - last[0]).hypot(first[1] - last[1]) < 1e-9);
                assert!(lap_track.points.iter().all(|point| {
                    (0..2).all(|axis_index| {
                        point[axis_index].abs() <= half_size[axis_index] - margin + 0.5
                    })
                }));
                let length_pixels = lap_track.length_pixels();
                let pace = length_pixels
                    / (LAP_PIXELS_PER_LOOP * f64::from(count_lap_loops(length_pixels)));
                assert!(pace > 0.5 && pace < 1.5, "{pond_size} seed {seed}: {pace}");
            }
        }
    }

    #[test]
    fn sizes_read_as_width_by_height_within_range() {
        assert_eq!(
            "768x256".parse::<PondSize>(),
            Ok(PondSize::new(768, 256).unwrap())
        );
        assert_eq!(PondSize::new(768, 256).unwrap().to_string(), "768x256");
        assert_eq!("256x256".parse::<PondSize>().unwrap(), PondSize::default());
        for malformed in [
            "",
            "768",
            "768x",
            "x256",
            "768X256",
            "-768x256",
            "76.8x256",
            "768x256x2",
        ] {
            assert!(
                matches!(
                    malformed.parse::<PondSize>(),
                    Err(PondSizeError::Malformed(_))
                ),
                "{malformed:?}"
            );
        }
        for out_of_range in ["255x256", "256x4097", "99999999999x256"] {
            assert!(
                matches!(
                    out_of_range.parse::<PondSize>(),
                    Err(PondSizeError::OutOfRange { .. })
                ),
                "{out_of_range:?}"
            );
        }
    }
}
