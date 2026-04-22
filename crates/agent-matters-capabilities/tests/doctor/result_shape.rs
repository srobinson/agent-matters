use crate::common::run_fixture;

#[test]
fn doctor_result_has_stable_json_shape() {
    let result = run_fixture("catalogs/valid");

    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(encoded["catalog"]["capability_count"], 6);
    assert_eq!(encoded["catalog"]["profile_count"], 1);
    assert_eq!(encoded["index"]["status"], "missing");
    assert!(encoded.get("runtimes").is_some());
    assert!(encoded.get("generated_state").is_some());
    assert_eq!(encoded["diagnostics"], serde_json::json!([]));
}
