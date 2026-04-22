use crate::common::run_fixture;

#[test]
fn runtime_adapter_registry_is_reported() {
    let result = run_fixture("catalogs/valid");

    let codex = result
        .runtimes
        .iter()
        .find(|runtime| runtime.id == "codex")
        .expect("codex runtime summary");
    let claude = result
        .runtimes
        .iter()
        .find(|runtime| runtime.id == "claude")
        .expect("claude runtime summary");

    assert!(codex.adapter_available);
    assert!(codex.default_config_valid);
    assert!(claude.adapter_available);
    assert!(claude.default_config_valid);
}
