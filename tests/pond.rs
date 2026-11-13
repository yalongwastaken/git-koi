use std::collections::BTreeSet;

use chrono::Days;
use git_koi::history::{AccountSnapshot, DayActivity};
use git_koi::koi::{KoiDevelopment, derive_koi_development};
use git_koi::pond::{
    FULL_WAKE_CONTRIBUTIONS, FULL_WATER_CONTRIBUTIONS, MAX_MARK_COUNT, PondObject, PondObjectKind,
    PondObjectSize, QUIET_WAKE_SCALE, SLOT_COUNT, VARIANT_COUNT, lay_out_pond,
};

#[test]
fn a_quiet_week_leaves_a_pond_without_objects() {
    for quiet_pond in [
        lay_out_pond(&develop_with_week(&[])),
        lay_out_pond(&develop_with_week(&[(0, 0), (0, 0), (0, 0)])),
    ] {
        assert!(quiet_pond.lily_pads.is_empty());
        assert!(quiet_pond.rocks.is_empty());
    }
}

#[test]
fn water_follows_the_weeks_and_the_days_contributions() {
    let water_for = |week_contributions: u64, day_contributions: u64| {
        let mut koi_development = develop_with_week(&[]);
        koi_development.recent_activity.recent_contributions = week_contributions;
        koi_development
            .recent_activity
            .observation_week_contributions = vec![day_contributions];
        lay_out_pond(&koi_development).water
    };
    assert_eq!(water_for(0, 0).mark_count, 0);
    assert!((2..=3).contains(&water_for(5, 0).mark_count));
    assert!((7..=9).contains(&water_for(20, 0).mark_count));
    assert_eq!(
        water_for(FULL_WATER_CONTRIBUTIONS, 0).mark_count,
        MAX_MARK_COUNT
    );
    assert_eq!(water_for(10_000, 0).mark_count, MAX_MARK_COUNT);
    assert!(
        (0..=FULL_WATER_CONTRIBUTIONS)
            .map(|week| water_for(week, 0).mark_count)
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[0] <= pair[1])
    );

    assert_eq!(water_for(0, 0).wake_scale, QUIET_WAKE_SCALE);
    let busy_day = water_for(0, FULL_WAKE_CONTRIBUTIONS).wake_scale;
    assert!((busy_day - 1.0).abs() < 1e-9);
    assert_eq!(water_for(0, 1_000).wake_scale, busy_day);
    let middling = water_for(0, 3).wake_scale;
    assert!(QUIET_WAKE_SCALE < middling && middling < 1.0);

    // Without any observed day, the wake is the quiet one.
    let mut unobserved = develop_with_week(&[]);
    unobserved
        .recent_activity
        .observation_week_contributions
        .clear();
    assert_eq!(lay_out_pond(&unobserved).water.wake_scale, QUIET_WAKE_SCALE);
}

#[test]
fn each_active_day_floats_a_lily_pad_for_issues_and_settles_a_rock_for_pull_requests() {
    // Oldest first: six days ago through the observation date.
    let pond = lay_out_pond(&develop_with_week(&[
        (1, 0),
        (0, 0),
        (0, 2),
        (3, 3),
        (0, 0),
        (0, 0),
        (4, 0),
    ]));

    let describe = |objects: &[PondObject]| -> Vec<(usize, u32, PondObjectSize)> {
        objects
            .iter()
            .map(|pond_object| (pond_object.slot, pond_object.age_days, pond_object.size))
            .collect()
    };
    assert_eq!(
        describe(&pond.lily_pads),
        [
            (0, 0, PondObjectSize::Large),
            (1, 3, PondObjectSize::Medium),
            (2, 6, PondObjectSize::Small),
        ]
    );
    assert_eq!(
        describe(&pond.rocks),
        [
            (0, 3, PondObjectSize::Medium),
            (1, 4, PondObjectSize::Medium)
        ]
    );
    assert!(
        pond.lily_pads
            .iter()
            .all(|pond_object| pond_object.kind == PondObjectKind::LilyPad)
    );
    assert!(
        pond.rocks
            .iter()
            .all(|pond_object| pond_object.kind == PondObjectKind::Rock)
    );
}

#[test]
fn size_follows_the_days_count() {
    for (day_count, expected_size) in [
        (1, PondObjectSize::Small),
        (2, PondObjectSize::Medium),
        (3, PondObjectSize::Medium),
        (4, PondObjectSize::Large),
        (90, PondObjectSize::Large),
    ] {
        let pond = lay_out_pond(&develop_with_week(&[(day_count, day_count)]));
        assert_eq!(pond.lily_pads[0].size, expected_size, "{day_count} issues");
        assert_eq!(
            pond.rocks[0].size, expected_size,
            "{day_count} pull requests"
        );
    }
}

#[test]
fn a_full_week_fits_in_the_slots() {
    let pond = lay_out_pond(&develop_with_week(&[(1, 1); 7]));
    assert_eq!(pond.lily_pads.len(), 7);
    assert_eq!(pond.rocks.len(), 7);
    assert!(
        pond.lily_pads
            .iter()
            .chain(&pond.rocks)
            .all(|pond_object| pond_object.slot < SLOT_COUNT)
    );
}

#[test]
fn variants_are_stable_per_account_and_differ_between_accounts() {
    let koi_development = develop_with_week(&[(1, 1); 7]);
    let pond = lay_out_pond(&koi_development);
    assert_eq!(lay_out_pond(&koi_development), pond);
    assert!(
        pond.lily_pads
            .iter()
            .chain(&pond.rocks)
            .all(|pond_object| pond_object.variant < VARIANT_COUNT)
    );

    let variants_for = |identity_seed: u64| -> Vec<u8> {
        let mut other_development = develop_with_week(&[(1, 1); 7]);
        other_development.identity_seed = identity_seed;
        let other_pond = lay_out_pond(&other_development);
        other_pond
            .lily_pads
            .iter()
            .chain(&other_pond.rocks)
            .map(|pond_object| pond_object.variant)
            .collect()
    };
    let distinct_ponds: BTreeSet<Vec<u8>> = (1..=12).map(variants_for).collect();
    assert!(distinct_ponds.len() > 1);
}

/// A development whose week is the given (issues, pull requests) pairs, oldest first,
/// ending on the observation date.
fn develop_with_week(week: &[(u32, u32)]) -> KoiDevelopment {
    let mut account_snapshot: AccountSnapshot =
        serde_json::from_str(include_str!("fixtures/account-with-pond.json")).unwrap();
    let observed_on = account_snapshot
        .contribution_history
        .as_ref()
        .unwrap()
        .observed_on;
    account_snapshot.daily_activity = Some(
        week.iter()
            .rev()
            .enumerate()
            .map(
                |(age_days, &(issues_opened, pull_requests_opened))| DayActivity {
                    date: observed_on - Days::new(age_days as u64),
                    issues_opened,
                    pull_requests_opened,
                },
            )
            .collect(),
    );
    derive_koi_development(&account_snapshot).unwrap()
}
