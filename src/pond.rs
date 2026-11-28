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
    /// The shortest side a pond may have: the default canvas, the one the koi is
    /// drawn for. In a smaller pond a grown koi reaches past the edge.
    pub const MIN_SIDE_PIXELS: u32 = FRAME_WIDTH_PIXELS;
    /// The largest area a pond may have. Rendering holds every frame of the loop at
    /// once, about frames × area × 4 bytes, and a longer pond swims a longer loop, so
    /// the area is capped until frames are streamed into the encoder.
    pub const MAX_AREA_PIXELS: u32 = 2048 * 256;
    /// The longest side a pond may have: the largest area at the shortest side.
    pub const MAX_SIDE_PIXELS: u32 = Self::MAX_AREA_PIXELS / Self::MIN_SIDE_PIXELS;

    /// A pond of this many native pixels across and down, each side at least
    /// [`Self::MIN_SIDE_PIXELS`] and the area at most [`Self::MAX_AREA_PIXELS`], so no
    /// side is longer than [`Self::MAX_SIDE_PIXELS`].
    pub fn new(width_pixels: u32, height_pixels: u32) -> Result<Self, PondSizeError> {
        let area_pixels = u64::from(width_pixels) * u64::from(height_pixels);
        if width_pixels >= Self::MIN_SIDE_PIXELS
            && height_pixels >= Self::MIN_SIDE_PIXELS
            && area_pixels <= u64::from(Self::MAX_AREA_PIXELS)
        {
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
            parse_whole_number(side_text)
                .ok_or_else(|| PondSizeError::Malformed(size_text.to_owned()))
        };
        Self::new(parse_side(width_text)?, parse_side(height_text)?)
    }
}

/// Why a pond size was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PondSizeError {
    /// The text is not `WIDTHxHEIGHT` in whole pixels; carries the text.
    Malformed(String),
    /// A side is shorter than [`PondSize::MIN_SIDE_PIXELS`], or the area is larger than
    /// [`PondSize::MAX_AREA_PIXELS`].
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
                "pond size {width_pixels}x{height_pixels} needs each side at least {} pixels \
                 and an area of at most {} pixels, as in {}x{}",
                PondSize::MIN_SIDE_PIXELS,
                PondSize::MAX_AREA_PIXELS,
                PondSize::MAX_SIDE_PIXELS,
                PondSize::MIN_SIDE_PIXELS
            ),
        }
    }
}

impl Error for PondSizeError {}

/// Read a whole number written in digits alone, with no sign or spaces; `None` when the
/// text is not one. Too many digits for a `u32` read as `u32::MAX`, so a range check
/// reports them as out of range rather than malformed.
pub(crate) fn parse_whole_number(number_text: &str) -> Option<u32> {
    if number_text.is_empty()
        || !number_text
            .bytes()
            .all(|number_byte| number_byte.is_ascii_digit())
    {
        return None;
    }
    Some(number_text.parse().unwrap_or(u32::MAX))
}

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
/// How many layouts to draw before settling for the plain oval: enough that even the
/// thinnest pond allowed rarely settles for it.
const TRACK_ATTEMPTS: u32 = 400;

/// One piece of track: a straight, or a turn along a circle.
#[derive(Clone, Copy)]
enum TrackMove {
    Straight {
        length_pixels: f64,
    },
    Turn {
        radius_pixels: f64,
        /// Clockwise on screen when positive.
        sweep_radians: f64,
    },
}

impl LapTrack {
    /// A track along the pond's long side whose widest turn is `widest_radius_pixels`,
    /// so the koi keeps the same margin from the long sides as from the short ones.
    /// Layouts that would leave the pond or stray from the koi's pace are drawn again;
    /// if none fits, the track is a plain oval.
    fn generate(seed: u64, pond_size: PondSize, widest_radius_pixels: f64) -> Self {
        let mut random = SplitMix64(seed);
        let long_side_pixels = pond_size.width_pixels.max(pond_size.height_pixels);
        let half_extent_pixels = [
            f64::from(long_side_pixels - pond_size.short_side_pixels()) / 2.0
                + widest_radius_pixels,
            widest_radius_pixels,
        ];
        let mut chosen_points = None;
        for _ in 0..TRACK_ATTEMPTS {
            let end_radius_pixels = random.uniform(
                (widest_radius_pixels - END_RADIUS_SPREAD_PIXELS).max(MIN_TURN_RADIUS_PIXELS),
                widest_radius_pixels,
            );
            // A figure-eight needs its lobes apart to cross between them.
            let has_room_for_eight = half_extent_pixels[0] - end_radius_pixels > end_radius_pixels;
            let track_layout = if random.chance(0.5) || !has_room_for_eight {
                draw_oval(&mut random, half_extent_pixels[0], end_radius_pixels)
            } else {
                draw_figure_eight(&mut random, half_extent_pixels[0], end_radius_pixels)
            };
            let mut points = trace(&track_layout);
            // Mirroring top to bottom swims the same layout the other way round.
            if random.chance(0.5) {
                for point in &mut points {
                    point[1] = -point[1];
                }
            }
            let stays_inside = points.iter().all(|point| {
                (0..2).all(|axis_index| {
                    point[axis_index].abs() <= half_extent_pixels[axis_index] + 0.5
                })
            });
            let eased_points = ease(&points);
            let length_pixels = measure_distances(&eased_points)[eased_points.len() - 1];
            let pace_ratio =
                length_pixels / (LAP_PIXELS_PER_LOOP * f64::from(count_lap_loops(length_pixels)));
            if stays_inside && (pace_ratio - 1.0).abs() <= PACE_TOLERANCE {
                chosen_points = Some(eased_points);
                break;
            }
        }
        let mut points = chosen_points.unwrap_or_else(|| {
            ease(&trace(&draw_plain_oval(
                half_extent_pixels[0],
                widest_radius_pixels,
            )))
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
        let wrapped_distance_pixels = distance_pixels.rem_euclid(self.length_pixels());
        let next_index = self
            .distances
            .partition_point(|&point_distance| point_distance <= wrapped_distance_pixels)
            .clamp(1, self.points.len() - 1);
        let [from_point, to_point] = [self.points[next_index - 1], self.points[next_index]];
        let [from_distance, to_distance] =
            [self.distances[next_index - 1], self.distances[next_index]];
        let segment_fraction = (wrapped_distance_pixels - from_distance)
            / (to_distance - from_distance).max(f64::EPSILON);
        [0, 1].map(|axis_index| {
            from_point[axis_index]
                + (to_point[axis_index] - from_point[axis_index]) * segment_fraction
        })
    }
}

/// Distance along the points to each one, from zero at the first.
fn measure_distances(points: &[[f64; 2]]) -> Vec<f64> {
    let mut distances = Vec::with_capacity(points.len());
    distances.push(0.0);
    for point_pair in points.windows(2) {
        let step_pixels =
            (point_pair[1][0] - point_pair[0][0]).hypot(point_pair[1][1] - point_pair[0][1]);
        distances.push(distances[distances.len() - 1] + step_pixels);
    }
    distances
}

/// The moves of a track, where it starts, and its heading there.
struct TrackLayout {
    moves: Vec<TrackMove>,
    start_point: [f64; 2],
    start_heading_radians: f64,
}

/// An oval with no loops or wiggles, swum clockwise from the top left.
fn draw_plain_oval(half_width_pixels: f64, end_radius_pixels: f64) -> TrackLayout {
    let straight_pixels = 2.0 * (half_width_pixels - end_radius_pixels);
    let end_turn = TrackMove::Turn {
        radius_pixels: end_radius_pixels,
        sweep_radians: PI,
    };
    TrackLayout {
        moves: vec![
            TrackMove::Straight {
                length_pixels: straight_pixels,
            },
            end_turn,
            TrackMove::Straight {
                length_pixels: straight_pixels,
            },
            end_turn,
        ],
        start_point: [-straight_pixels / 2.0, -end_radius_pixels],
        start_heading_radians: 0.0,
    }
}

/// An oval swum clockwise from the top left: two straights joined by half turns,
/// with features along both straights.
fn draw_oval(
    random: &mut SplitMix64,
    half_width_pixels: f64,
    end_radius_pixels: f64,
) -> TrackLayout {
    let straight_pixels = 2.0 * (half_width_pixels - end_radius_pixels);
    let end_turn = TrackMove::Turn {
        radius_pixels: end_radius_pixels,
        sweep_radians: PI,
    };
    let mut moves = place_features(random, straight_pixels);
    moves.push(end_turn);
    moves.extend(place_features(random, straight_pixels));
    moves.push(end_turn);
    TrackLayout {
        moves,
        start_point: [-straight_pixels / 2.0, -end_radius_pixels],
        start_heading_radians: 0.0,
    }
}

/// A figure-eight from the center, heading down and right: counterclockwise round the
/// right lobe and clockwise round the left, crossing in the middle, with features
/// along each half of both crossing lines.
fn draw_figure_eight(
    random: &mut SplitMix64,
    half_width_pixels: f64,
    end_radius_pixels: f64,
) -> TrackLayout {
    let lobe_offset_pixels = half_width_pixels - end_radius_pixels;
    let crossing_angle_radians = (end_radius_pixels / lobe_offset_pixels).asin();
    let half_crossing_pixels =
        (lobe_offset_pixels * lobe_offset_pixels - end_radius_pixels * end_radius_pixels).sqrt();
    let lobe_sweep_radians = PI + 2.0 * crossing_angle_radians;
    let mut moves = place_features(random, half_crossing_pixels);
    moves.push(TrackMove::Turn {
        radius_pixels: end_radius_pixels,
        sweep_radians: -lobe_sweep_radians,
    });
    moves.extend(place_features(random, half_crossing_pixels));
    moves.extend(place_features(random, half_crossing_pixels));
    moves.push(TrackMove::Turn {
        radius_pixels: end_radius_pixels,
        sweep_radians: lobe_sweep_radians,
    });
    moves.extend(place_features(random, half_crossing_pixels));
    TrackLayout {
        moves,
        start_point: [0.0, 0.0],
        start_heading_radians: crossing_angle_radians,
    }
}

/// Loop-the-loops or S-wiggles along a straight, up to two or one per FEATURE_SPACING
/// pixels, whichever is more, each curling either way and spaced so none overlap; the
/// plain straight when they don't fit.
fn place_features(random: &mut SplitMix64, straight_pixels: f64) -> Vec<TrackMove> {
    const FEATURE_SPACING_PIXELS: f64 = 300.0;
    let max_feature_count = ((straight_pixels / FEATURE_SPACING_PIXELS) as u64).max(2);
    let feature_count = random.next() % (max_feature_count + 1);
    let features: Vec<TrackFeature> = (0..feature_count)
        .map(|_| TrackFeature::draw(random))
        .collect();
    let Some(gap_lengths_pixels) = space_features(random, straight_pixels, &features) else {
        return vec![TrackMove::Straight {
            length_pixels: straight_pixels,
        }];
    };
    let mut moves = Vec::new();
    for (feature, &gap_length_pixels) in features.iter().zip(&gap_lengths_pixels) {
        moves.push(TrackMove::Straight {
            length_pixels: gap_length_pixels,
        });
        moves.extend(feature.moves());
    }
    moves.push(TrackMove::Straight {
        length_pixels: gap_lengths_pixels[features.len()],
    });
    moves
}

/// The straight before, between, and after the features, one more gap than there are
/// features, each a random share of the length the features leave free; `None` when the
/// features would overlap or crowd the straight's ends. With no features there is
/// nothing to space, so nothing is drawn and the one gap is the whole straight.
fn space_features(
    random: &mut SplitMix64,
    straight_pixels: f64,
    features: &[TrackFeature],
) -> Option<Vec<f64>> {
    // Room left clear at each end of the straight, so features don't crowd turns.
    const END_CLEARANCE_PIXELS: f64 = 12.0;
    // Every gap gets some straight: the shortest is at least 0.3 / 1.3 of the longest.
    const GAP_WEIGHT_RANGE: (f64, f64) = (0.3, 1.3);
    if features.is_empty() {
        return Some(vec![straight_pixels]);
    }
    let free_length_pixels = straight_pixels
        - features
            .iter()
            .map(|feature| feature.advance_pixels())
            .sum::<f64>();
    let gap_weights: Vec<f64> = (0..=features.len())
        .map(|_| random.uniform(GAP_WEIGHT_RANGE.0, GAP_WEIGHT_RANGE.1))
        .collect();
    let gap_weight_total: f64 = gap_weights.iter().sum();
    let gap_lengths_pixels: Vec<f64> = gap_weights
        .iter()
        .map(|gap_weight| free_length_pixels * gap_weight / gap_weight_total)
        .collect();
    let mut along_pixels = 0.0;
    let mut feature_spans = Vec::with_capacity(features.len());
    for (feature, gap_length_pixels) in features.iter().zip(&gap_lengths_pixels) {
        along_pixels += gap_length_pixels;
        let middle_pixels = along_pixels + feature.advance_pixels() / 2.0;
        feature_spans.push(
            middle_pixels - feature.reach_pixels() / 2.0
                ..middle_pixels + feature.reach_pixels() / 2.0,
        );
        along_pixels += feature.advance_pixels();
    }
    let features_fit = free_length_pixels > 0.0
        && feature_spans[0].start > END_CLEARANCE_PIXELS
        && feature_spans[feature_spans.len() - 1].end < straight_pixels - END_CLEARANCE_PIXELS
        && feature_spans
            .windows(2)
            .all(|span_pair| span_pair[0].end < span_pair[1].start);
    features_fit.then_some(gap_lengths_pixels)
}

/// A loop-the-loop or an S-wiggle along a straight. A loop circles back to where it
/// began and a wiggle returns to the straight's line, so features never change where
/// the track goes.
#[derive(Clone, Copy)]
enum TrackFeature {
    /// One full turn.
    Loop {
        radius_pixels: f64,
        /// 1.0 to curl clockwise on screen, -1.0 counterclockwise.
        curl_sign: f64,
    },
    /// A turn out by an angle, back across by twice it, and back onto the line by it
    /// again.
    Wiggle {
        radius_pixels: f64,
        angle_radians: f64,
        /// 1.0 to curl clockwise on screen first, -1.0 counterclockwise.
        curl_sign: f64,
    },
}

impl TrackFeature {
    const LOOP_SHARE: f64 = 0.55;
    const MAX_LOOP_RADIUS_PIXELS: f64 = 64.0;
    const MAX_WIGGLE_RADIUS_PIXELS: f64 = 80.0;
    const WIGGLE_ANGLE_RANGE_RADIANS: (f64, f64) = (0.5, 1.05);
    const LOOP_CLEARANCE_PIXELS: f64 = 16.0;
    const WIGGLE_CLEARANCE_PIXELS: f64 = 8.0;

    /// Draw a feature: which way it curls, whether it is a loop, then its size.
    fn draw(random: &mut SplitMix64) -> Self {
        let curl_sign = if random.chance(0.5) { 1.0 } else { -1.0 };
        if random.chance(Self::LOOP_SHARE) {
            let radius_pixels =
                random.uniform(MIN_TURN_RADIUS_PIXELS, Self::MAX_LOOP_RADIUS_PIXELS);
            Self::Loop {
                radius_pixels,
                curl_sign,
            }
        } else {
            let radius_pixels =
                random.uniform(MIN_TURN_RADIUS_PIXELS, Self::MAX_WIGGLE_RADIUS_PIXELS);
            let angle_radians = random.uniform(
                Self::WIGGLE_ANGLE_RANGE_RADIANS.0,
                Self::WIGGLE_ANGLE_RANGE_RADIANS.1,
            );
            Self::Wiggle {
                radius_pixels,
                angle_radians,
                curl_sign,
            }
        }
    }

    /// How far along the straight the feature carries the koi: none for a loop.
    fn advance_pixels(self) -> f64 {
        match self {
            Self::Loop { .. } => 0.0,
            Self::Wiggle {
                radius_pixels,
                angle_radians,
                ..
            } => 4.0 * radius_pixels * angle_radians.sin(),
        }
    }

    /// How much of the straight the feature covers, with room to spare, centered on the
    /// middle of its advance.
    fn reach_pixels(self) -> f64 {
        match self {
            Self::Loop { radius_pixels, .. } => 2.0 * radius_pixels + Self::LOOP_CLEARANCE_PIXELS,
            Self::Wiggle { .. } => self.advance_pixels() + Self::WIGGLE_CLEARANCE_PIXELS,
        }
    }

    /// The turns that swim the feature.
    fn moves(self) -> Vec<TrackMove> {
        match self {
            Self::Loop {
                radius_pixels,
                curl_sign,
            } => vec![TrackMove::Turn {
                radius_pixels,
                sweep_radians: curl_sign * TAU,
            }],
            Self::Wiggle {
                radius_pixels,
                angle_radians,
                curl_sign,
            } => vec![
                TrackMove::Turn {
                    radius_pixels,
                    sweep_radians: curl_sign * angle_radians,
                },
                TrackMove::Turn {
                    radius_pixels,
                    sweep_radians: -2.0 * curl_sign * angle_radians,
                },
                TrackMove::Turn {
                    radius_pixels,
                    sweep_radians: curl_sign * angle_radians,
                },
            ],
        }
    }
}

/// Follow a layout's moves from its start point and heading, one pixel or less per
/// point.
fn trace(track_layout: &TrackLayout) -> Vec<[f64; 2]> {
    let mut points = vec![track_layout.start_point];
    let mut heading_radians = track_layout.start_heading_radians;
    for &track_move in &track_layout.moves {
        let [x, y] = points[points.len() - 1];
        match track_move {
            TrackMove::Straight { length_pixels } => {
                let step_count = length_pixels.ceil().max(1.0) as usize;
                points.extend((1..=step_count).map(|step_index| {
                    let along_pixels = length_pixels * step_index as f64 / step_count as f64;
                    [
                        x + along_pixels * heading_radians.cos(),
                        y + along_pixels * heading_radians.sin(),
                    ]
                }));
            }
            TrackMove::Turn {
                radius_pixels,
                sweep_radians,
            } => {
                // The center sits on the side the koi turns toward.
                let turn_side_sign = sweep_radians.signum();
                let center_point = [
                    x - turn_side_sign * radius_pixels * heading_radians.sin(),
                    y + turn_side_sign * radius_pixels * heading_radians.cos(),
                ];
                let start_angle_radians = heading_radians - turn_side_sign * PI / 2.0;
                let step_count = (sweep_radians.abs() * radius_pixels).ceil().max(1.0) as usize;
                points.extend((1..=step_count).map(|step_index| {
                    let angle_radians =
                        start_angle_radians + sweep_radians * step_index as f64 / step_count as f64;
                    [
                        center_point[0] + radius_pixels * angle_radians.cos(),
                        center_point[1] + radius_pixels * angle_radians.sin(),
                    ]
                }));
                heading_radians += sweep_radians;
            }
        }
    }
    points
}

/// Respace a closed track's points one pixel apart, then blur them along the track,
/// so each change of turn spreads over EASING_PIXELS instead of snapping. A closed
/// track stays closed, and its turns tighten only slightly.
fn ease(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let traced_track = LapTrack::from_points(points.to_vec());
    let point_count = traced_track.length_pixels().round().max(3.0) as usize;
    let spacing_pixels = traced_track.length_pixels() / point_count as f64;
    let even_points: Vec<[f64; 2]> = (0..point_count)
        .map(|point_index| traced_track.position_at(point_index as f64 * spacing_pixels))
        .collect();
    let blur_reach_steps = (3.0 * EASING_PIXELS / spacing_pixels).ceil() as isize;
    let blur_weights: Vec<f64> = (-blur_reach_steps..=blur_reach_steps)
        .map(|step_offset| {
            (-0.5 * (step_offset as f64 * spacing_pixels / EASING_PIXELS).powi(2)).exp()
        })
        .collect();
    let blur_weight_total: f64 = blur_weights.iter().sum();
    let mut eased_points: Vec<[f64; 2]> = (0..point_count as isize)
        .map(|point_index| {
            let mut weighted_sum = [0.0; 2];
            for (blur_weight, step_offset) in blur_weights
                .iter()
                .zip(-blur_reach_steps..=blur_reach_steps)
            {
                let neighbor_point = even_points
                    [(point_index + step_offset).rem_euclid(point_count as isize) as usize];
                weighted_sum[0] += blur_weight * neighbor_point[0];
                weighted_sum[1] += blur_weight * neighbor_point[1];
            }
            weighted_sum.map(|axis_total| axis_total / blur_weight_total)
        })
        .collect();
    eased_points.push(eased_points[0]);
    eased_points
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
            (PondSize::new(1024, 512).unwrap(), 40.0),
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
        let shortest_side = PondSize::MIN_SIDE_PIXELS;
        let longest_side = PondSize::MAX_SIDE_PIXELS;
        let widest_square_side = f64::from(PondSize::MAX_AREA_PIXELS).sqrt() as u32;
        for (width_pixels, height_pixels) in [
            (longest_side, shortest_side),
            (shortest_side, longest_side),
            (widest_square_side, widest_square_side),
        ] {
            assert!(
                PondSize::new(width_pixels, height_pixels).is_ok(),
                "{width_pixels}x{height_pixels}"
            );
        }
        for (width_pixels, height_pixels) in [
            (shortest_side - 1, shortest_side),
            (longest_side + 1, shortest_side),
            (widest_square_side + 1, widest_square_side + 1),
        ] {
            assert!(
                matches!(
                    PondSize::new(width_pixels, height_pixels),
                    Err(PondSizeError::OutOfRange { .. })
                ),
                "{width_pixels}x{height_pixels}"
            );
        }
        assert!(matches!(
            "99999999999x256".parse::<PondSize>(),
            Err(PondSizeError::OutOfRange { .. })
        ));
    }
}
