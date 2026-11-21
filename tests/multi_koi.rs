//! Integration tests for the multi-koi scene module.

use git_koi::behavior::Behavior;
use git_koi::coat::KoiBody;
use git_koi::koi::derive_koi_development;
use git_koi::multi_koi::{KoiInstance, MultiKoiScene};
use git_koi::pond::PondSize;
use git_koi::render::SwimLoop;

/// Shared fixture: derive a [`git_koi::koi::KoiDevelopment`] from the account that already
/// carries contribution history, so it is usable as-is without patching fields.
fn fixture_development() -> git_koi::koi::KoiDevelopment {
    let account_snapshot =
        serde_json::from_str(include_str!("fixtures/account-with-history.json")).unwrap();
    derive_koi_development(&account_snapshot).unwrap()
}

/// A one-koi [`MultiKoiScene`] must render every frame identically to the
/// underlying [`SwimLoop`] rendered directly at the same (wrapped) frame index.
///
/// Both are constructed with [`Behavior::Cruise`] so the frame count is fully
/// determined by the koi development and pond, and there is no ambiguity from
/// behavior selection.
#[test]
fn single_koi_scene_matches_single_koi_swim_loop() {
    let koi_development = fixture_development();
    let pond_size = PondSize::default();
    let koi_body = KoiBody::default();

    let instance =
        KoiInstance::new_with_behavior(&koi_development, koi_body, Behavior::Cruise, pond_size);
    let frame_count = instance.frame_count();

    let swim_loop = SwimLoop::new(&koi_development, koi_body, Behavior::Cruise, pond_size);

    let scene = MultiKoiScene::new(vec![instance], pond_size);

    // The scene's frame count must equal the single koi's frame count.
    assert_eq!(scene.frame_count(), frame_count);

    // Every rendered frame must match the direct SwimLoop output.
    for scene_frame_index in 0..frame_count {
        let scene_frame = scene.render_frame(scene_frame_index);
        // The KoiInstance wraps scene_frame_index % frame_count, so the
        // SwimLoop should be called with the same wrapped index.
        let direct_frame = swim_loop.render_frame(scene_frame_index % frame_count);
        assert_eq!(
            scene_frame.as_rgba_bytes(),
            direct_frame.as_rgba_bytes(),
            "frame {scene_frame_index} differed between scene and direct SwimLoop"
        );
    }
}

/// A scene with two koi whose behaviors have fixed, coprime-ish frame counts
/// must report a frame count equal to the LCM of those individual counts.
///
/// [`Behavior::Rest`] always loops in 90 frames and [`Behavior::DeepRest`]
/// in 120 frames.  LCM(90, 120) = 360, which is below the 480-frame cap,
/// so the scene must report exactly 360.
#[test]
fn two_koi_scene_frame_count_is_lcm_capped() {
    let koi_development = fixture_development();
    let pond_size = PondSize::default();
    let koi_body = KoiBody::default();

    let rest_instance =
        KoiInstance::new_with_behavior(&koi_development, koi_body, Behavior::Rest, pond_size);
    let deep_rest_instance =
        KoiInstance::new_with_behavior(&koi_development, koi_body, Behavior::DeepRest, pond_size);

    // Confirm each koi carries the frame count we expect from the behavior constants.
    assert_eq!(
        rest_instance.frame_count(),
        90,
        "Rest should loop in 90 frames"
    );
    assert_eq!(
        deep_rest_instance.frame_count(),
        120,
        "DeepRest should loop in 120 frames"
    );

    let scene = MultiKoiScene::new(vec![rest_instance, deep_rest_instance], pond_size);

    // LCM(90, 120) = 360; below the 480-frame cap.
    assert_eq!(
        scene.frame_count(),
        360,
        "scene frame count must equal LCM(90, 120) = 360"
    );
}

/// Calling [`MultiKoiScene::render_frame`] twice with the same scene frame
/// index must return byte-for-byte identical frames.
///
/// This guards against any accidental mutation, random jitter, or
/// time-dependent rendering that would make composited output non-reproducible.
#[test]
fn composite_is_deterministic() {
    let koi_development = fixture_development();
    let pond_size = PondSize::default();
    let koi_body = KoiBody::default();

    // Two koi with different behaviors so the compositor actually exercises
    // multi-layer blending.
    let instance_a =
        KoiInstance::new_with_behavior(&koi_development, koi_body, Behavior::Cruise, pond_size);
    let instance_b =
        KoiInstance::new_with_behavior(&koi_development, koi_body, Behavior::Rest, pond_size);

    let scene = MultiKoiScene::new(vec![instance_a, instance_b], pond_size);

    for scene_frame_index in [0, 1, scene.frame_count() / 2, scene.frame_count() - 1] {
        let first_render = scene.render_frame(scene_frame_index);
        let second_render = scene.render_frame(scene_frame_index);
        assert_eq!(
            first_render.as_rgba_bytes(),
            second_render.as_rgba_bytes(),
            "render_frame({scene_frame_index}) was not deterministic"
        );
    }
}

/// An empty [`MultiKoiScene`] must report a frame count of 1 so callers
/// always have at least one renderable frame.
#[test]
fn scene_frame_count_is_one_when_no_koi() {
    let scene = MultiKoiScene::new(vec![], PondSize::default());
    assert_eq!(
        scene.frame_count(),
        1,
        "empty scene must have a frame count of 1"
    );
}
