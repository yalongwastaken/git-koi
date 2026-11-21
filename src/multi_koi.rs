//! Multiple koi in one pond, composited front-to-back.
//!
//! [`KoiInstance`] wraps a single koi's swimming loop in a shared pond.
//! [`MultiKoiScene`] holds several instances and composites their frames into one
//! per tick of the scene's clock.
//!
//! The scene's frame count is the LCM of its koi frame counts, capped at
//! [`MAX_SCENE_FRAMES`]. Each koi loops its own animation independently: its
//! frame at a given scene tick is `scene_frame_index % koi.frame_count()`.

use crate::behavior::{Behavior, select_behavior};
use crate::coat::KoiBody;
use crate::koi::KoiDevelopment;
use crate::pond::PondSize;
use crate::render::{KoiFrame, SwimLoop};

/// The scene ends at this many frames even when the LCM of its koi would be larger.
const MAX_SCENE_FRAMES: u32 = 480;

/// One koi in a multi-koi scene: a behavior loop ready to render on demand.
///
/// Holds a [`SwimLoop`] prepared for the koi's development and pond, so the
/// shared framing work is done once rather than on every rendered frame.
pub struct KoiInstance<'a> {
    swim_loop: SwimLoop<'a>,
}

impl<'a> KoiInstance<'a> {
    /// Prepare a koi for a scene, selecting the behavior automatically from its development.
    ///
    /// Calls [`crate::behavior::select_behavior`] on `koi_development` and delegates to
    /// [`Self::new_with_behavior`].
    pub fn new(
        koi_development: &'a KoiDevelopment,
        koi_body: KoiBody,
        pond_size: PondSize,
    ) -> Self {
        let behavior = select_behavior(koi_development);
        Self::new_with_behavior(koi_development, koi_body, behavior, pond_size)
    }

    /// Prepare a koi for a scene with an explicitly chosen behavior.
    pub fn new_with_behavior(
        koi_development: &'a KoiDevelopment,
        koi_body: KoiBody,
        behavior: Behavior,
        pond_size: PondSize,
    ) -> Self {
        Self {
            swim_loop: SwimLoop::new(koi_development, koi_body, behavior, pond_size),
        }
    }

    /// The number of frames in this koi's swimming loop.
    pub fn frame_count(&self) -> u32 {
        self.swim_loop.frame_count()
    }

    /// Render the frame of this koi's loop that falls at `scene_frame_index`.
    ///
    /// Maps the scene index via `scene_frame_index % self.frame_count()` so the
    /// koi loops continuously regardless of how long the scene runs.
    pub fn render_frame(&self, scene_frame_index: u32) -> KoiFrame {
        self.swim_loop
            .render_frame(scene_frame_index % self.frame_count())
    }
}

/// A group of koi in one pond, composited front-to-back.
///
/// Koi earlier in the slice appear in front of later ones. The scene's frame
/// count is the LCM of all koi frame counts, capped at [`MAX_SCENE_FRAMES`];
/// an empty scene has a frame count of 1. The count is computed once at
/// construction and stored, so [`Self::render_frame`] never recomputes it.
pub struct MultiKoiScene<'a> {
    koi_instances: Vec<KoiInstance<'a>>,
    pond_size: PondSize,
    scene_frame_count: u32,
}

impl<'a> MultiKoiScene<'a> {
    /// Create a scene from a list of koi instances and a shared pond size.
    ///
    /// `scene_frame_count` is computed immediately as the LCM of all instance
    /// frame counts, capped at [`MAX_SCENE_FRAMES`]. An empty `koi_instances`
    /// list produces a scene with a frame count of 1.
    pub fn new(koi_instances: Vec<KoiInstance<'a>>, pond_size: PondSize) -> Self {
        let scene_frame_count = compute_scene_frame_count(&koi_instances);
        Self {
            koi_instances,
            pond_size,
            scene_frame_count,
        }
    }

    /// The number of frames in the scene's loop.
    pub fn frame_count(&self) -> u32 {
        self.scene_frame_count
    }

    /// The size of the pond this scene is drawn in.
    pub fn pond_size(&self) -> PondSize {
        self.pond_size
    }

    /// Render one frame of the scene by compositing all koi front-to-back.
    ///
    /// Each koi's scene frame index is mapped through its own loop length, so
    /// koi with different loop lengths cycle independently. The first koi in the
    /// scene is closest to the viewer; each later one fills only pixels left
    /// transparent by those in front of it.
    pub fn render_frame(&self, scene_frame_index: u32) -> KoiFrame {
        let frames: Vec<KoiFrame> = self
            .koi_instances
            .iter()
            .map(|instance| instance.render_frame(scene_frame_index))
            .collect();
        let frame_refs: Vec<&KoiFrame> = frames.iter().collect();
        KoiFrame::composite(&frame_refs)
    }
}

/// Compute the scene frame count: LCM of all instance frame counts, capped at
/// [`MAX_SCENE_FRAMES`]. Returns 1 for an empty slice.
fn compute_scene_frame_count(koi_instances: &[KoiInstance<'_>]) -> u32 {
    if koi_instances.is_empty() {
        return 1;
    }
    let mut lcm_so_far: u64 = 1;
    for instance in koi_instances {
        let count = u64::from(instance.frame_count());
        lcm_so_far = lcm(lcm_so_far, count);
        if lcm_so_far >= u64::from(MAX_SCENE_FRAMES) {
            return MAX_SCENE_FRAMES;
        }
    }
    lcm_so_far as u32
}

/// Greatest common divisor of two integers.
fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Least common multiple of two integers. Returns 0 when either argument is 0.
fn lcm(a: u64, b: u64) -> u64 {
    if a == 0 || b == 0 {
        return 0;
    }
    // Divide before multiplying to reduce overflow risk.
    (a / gcd(a, b)).saturating_mul(b)
}
