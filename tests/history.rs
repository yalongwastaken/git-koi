use std::collections::BTreeMap;

use git_koi::history::{
    AccountSnapshot, ConflictingRepository, RepositoryVisibility, calculate_language_shares,
};

const ACCOUNT_JSON: &str = include_str!("fixtures/account.json");

#[test]
fn language_shares_count_only_eligible_projects() {
    let account_snapshot: AccountSnapshot = serde_json::from_str(ACCOUNT_JSON).unwrap();

    let language_shares = calculate_language_shares(&account_snapshot).unwrap();

    assert_eq!(
        language_shares,
        BTreeMap::from([
            ("C++".to_owned(), 0.2),
            ("Python".to_owned(), 0.2),
            ("Rust".to_owned(), 0.6),
        ])
    );
}

#[test]
fn language_shares_stay_identical_after_reordering_and_duplicate_observations() {
    let mut account_snapshot: AccountSnapshot = serde_json::from_str(ACCOUNT_JSON).unwrap();
    let original_shares = calculate_language_shares(&account_snapshot).unwrap();
    account_snapshot
        .repositories
        .push(account_snapshot.repositories[0].clone());
    account_snapshot.repositories.reverse();

    for _ in 0..account_snapshot.repositories.len() {
        account_snapshot.repositories.rotate_left(1);
        assert_eq!(
            calculate_language_shares(&account_snapshot).unwrap(),
            original_shares
        );
    }
}

#[test]
fn missing_observations_cannot_be_read_as_a_confirmed_empty_collection() {
    let mut snapshot_document: serde_json::Value = serde_json::from_str(ACCOUNT_JSON).unwrap();
    snapshot_document["repositories"] = serde_json::json!([]);
    let empty_snapshot: AccountSnapshot =
        serde_json::from_value(snapshot_document.clone()).unwrap();
    assert!(
        calculate_language_shares(&empty_snapshot)
            .unwrap()
            .is_empty()
    );

    snapshot_document
        .as_object_mut()
        .unwrap()
        .remove("repositories");
    assert!(serde_json::from_value::<AccountSnapshot>(snapshot_document).is_err());
}

#[test]
fn missing_language_cannot_be_read_as_a_known_absence() {
    let mut snapshot_document: serde_json::Value = serde_json::from_str(ACCOUNT_JSON).unwrap();
    snapshot_document["repositories"][0]["primary_language"] = serde_json::Value::Null;
    let null_language_snapshot: AccountSnapshot =
        serde_json::from_value(snapshot_document.clone()).unwrap();
    assert_eq!(
        calculate_language_shares(&null_language_snapshot).unwrap()["Rust"],
        0.5
    );

    snapshot_document["repositories"][0]
        .as_object_mut()
        .unwrap()
        .remove("primary_language");
    assert!(serde_json::from_value::<AccountSnapshot>(snapshot_document).is_err());
}

#[test]
fn conflicting_observations_fail_regardless_of_order_or_eligibility() {
    let account_snapshot: AccountSnapshot = serde_json::from_str(ACCOUNT_JSON).unwrap();
    let mut repository_with_conflicting_language = account_snapshot.repositories[0].clone();
    repository_with_conflicting_language.primary_language = Some("Python".to_owned());
    let mut repository_with_conflicting_visibility = account_snapshot.repositories[0].clone();
    repository_with_conflicting_visibility.visibility = RepositoryVisibility::Private;

    for conflicting_observation in [
        repository_with_conflicting_language,
        repository_with_conflicting_visibility,
    ] {
        let expected_error = ConflictingRepository {
            repository_id: conflicting_observation.repository_id,
        };
        let mut conflicting_snapshot = account_snapshot.clone();
        conflicting_snapshot
            .repositories
            .push(conflicting_observation);
        assert_eq!(
            calculate_language_shares(&conflicting_snapshot),
            Err(expected_error)
        );
        conflicting_snapshot.repositories.reverse();
        assert!(calculate_language_shares(&conflicting_snapshot).is_err());
    }
}

#[test]
fn snapshot_writer_and_reader_preserve_observations_and_language_shares() {
    let account_snapshot: AccountSnapshot = serde_json::from_str(ACCOUNT_JSON).unwrap();
    let snapshot_bytes = serde_json::to_vec(&account_snapshot).unwrap();
    let restored_snapshot: AccountSnapshot = serde_json::from_slice(&snapshot_bytes).unwrap();

    assert_eq!(restored_snapshot, account_snapshot);
    assert_eq!(
        calculate_language_shares(&restored_snapshot).unwrap(),
        calculate_language_shares(&account_snapshot).unwrap()
    );
}

#[test]
fn incompatible_documents_cannot_enter_language_calculation() {
    let snapshot_document: serde_json::Value = serde_json::from_str(ACCOUNT_JSON).unwrap();
    let mut snapshot_with_unsupported_version = snapshot_document.clone();
    snapshot_with_unsupported_version["schema_version"] = serde_json::json!(999);
    let mut snapshot_without_version = snapshot_document.clone();
    snapshot_without_version
        .as_object_mut()
        .unwrap()
        .remove("schema_version");
    let mut snapshot_with_unknown_account_field = snapshot_document.clone();
    snapshot_with_unknown_account_field["unexpected_field"] = serde_json::json!(true);
    let mut snapshot_with_unknown_repository_field = snapshot_document;
    snapshot_with_unknown_repository_field["repositories"][0]["unexpected_field"] =
        serde_json::json!(true);

    for incompatible_document in [
        snapshot_with_unsupported_version,
        snapshot_without_version,
        snapshot_with_unknown_account_field,
        snapshot_with_unknown_repository_field,
    ] {
        assert!(serde_json::from_value::<AccountSnapshot>(incompatible_document).is_err());
    }
}
