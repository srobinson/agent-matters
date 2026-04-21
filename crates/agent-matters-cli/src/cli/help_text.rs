//! Hand authored help text. Works alongside the generated constants in
//! [`super::generated_help`]: the generator emits `{PREFIX}_ABOUT` and
//! per-argument help from `tools.toml`, this module carries example
//! oriented `{PREFIX}_AFTER_HELP` blocks and the top level CLI narrative.
//!
//! Examples intentionally stay plain ASCII for MVP. When ANSI coloring
//! lands it should come in via a single `color_print::cstr!` wrap here
//! without changing how the constants are consumed by the clap modules.

#[rustfmt::skip]
pub const LONG_ABOUT: &str = "agent-matters compiles selected capabilities, instructions, hooks, \
MCP servers, runtime settings, and launch material into focused runtime \
homes for Codex, Claude, and future CLI runtimes.\n\n\
Runtime homes (`.codex`, `.claude`) are generated rather than hand \
maintained source of truth. Author capabilities and profiles once; compile \
and activate them per runtime.";

#[rustfmt::skip]
pub const PROFILES_LIST_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters profiles list\n  \
  agent-matters profiles list --json";

#[rustfmt::skip]
pub const PROFILES_SHOW_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters profiles show my-profile\n  \
  agent-matters profiles show my-profile --json";

#[rustfmt::skip]
pub const PROFILES_COMPILE_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters profiles compile my-profile --runtime codex\n  \
  agent-matters profiles compile my-profile --runtime claude";

#[rustfmt::skip]
pub const PROFILES_USE_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters profiles use my-profile --runtime codex\n  \
  agent-matters profiles use my-profile ./some/repo --runtime claude";

#[rustfmt::skip]
pub const CAPABILITIES_LIST_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters capabilities list\n  \
  agent-matters capabilities list --json";

#[rustfmt::skip]
pub const CAPABILITIES_SHOW_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters capabilities show helioy/mail\n  \
  agent-matters capabilities show helioy/mail --json";

#[rustfmt::skip]
pub const CAPABILITIES_DIFF_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters capabilities diff helioy/mail\n  \
  agent-matters capabilities diff helioy/mail --json";

#[rustfmt::skip]
pub const SOURCES_SEARCH_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters sources search skills.sh 'terraform review'\n  \
  agent-matters sources search skills.sh 'terraform review' --json";

#[rustfmt::skip]
pub const SOURCES_IMPORT_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters sources import skills.sh://author/name@1.2.0";

#[rustfmt::skip]
pub const DOCTOR_AFTER_HELP: &str =
"Examples:\n  \
  agent-matters doctor\n  \
  agent-matters doctor --json";
