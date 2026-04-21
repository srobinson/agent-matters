use agent_matters_core::catalog::{CapabilityIndexRecord, ProfileIndexRecord};
use agent_matters_core::runtime::{BUILD_PLAN_SCHEMA_VERSION, FingerprintBuilder};
use serde::Serialize;

use super::super::{
    BuildPlanInstructionOutput, ResolvedInstructionFragment, ResolvedRuntimeConfig,
};
use super::inputs::ReadContentInput;

pub(super) fn build_fingerprint(
    profile_record: &ProfileIndexRecord,
    effective_capabilities: &[CapabilityIndexRecord],
    instruction_fragments: &[ResolvedInstructionFragment],
    instruction_output: &BuildPlanInstructionOutput,
    runtime_config: &ResolvedRuntimeConfig,
    adapter_version: &str,
    read_inputs: &[ReadContentInput],
) -> String {
    let mut hasher = FingerprintBuilder::new(BUILD_PLAN_SCHEMA_VERSION);
    write_json(&mut hasher, "profile-record", profile_record);
    write_json(
        &mut hasher,
        "effective-capabilities",
        effective_capabilities,
    );
    write_json(&mut hasher, "instruction-fragments", instruction_fragments);
    write_json(&mut hasher, "instruction-output", instruction_output);
    write_json(&mut hasher, "runtime-config", runtime_config);
    hasher.write_str("adapter-version");
    hasher.write_str(adapter_version);
    for input in read_inputs {
        hasher.write_str("content-input");
        hasher.write_str(&input.content_input.role);
        hasher.write_str(&input.content_input.path);
        hasher.write_bytes(&input.bytes);
    }
    hasher.finish_prefixed()
}

fn write_json<T: Serialize + ?Sized>(hasher: &mut FingerprintBuilder, label: &str, value: &T) {
    let encoded = serde_json::to_vec(value).expect("build plan fingerprint material serializes");
    hasher.write_str(label);
    hasher.write_bytes(&encoded);
}
