// Re-export Crucible's full public surface.
pub use crucible_fuzzer::*;
pub use crucible_test_context::{
    clear_violation_tracking, get_violation_action_index, has_violation, record_violation,
    set_violation_action_index, take_violation, TxError, TxOutcome,
};

pub mod capability;
pub use capability::{HasContext, HasInstructionSet, InstructionSpec};

pub mod prelude {
    pub use crate::capability::{HasContext, HasInstructionSet, InstructionSpec};
    pub use crucible_fuzzer::*;
    pub use crucible_test_context::{
        clear_violation_tracking, get_violation_action_index, has_violation, record_violation,
        set_violation_action_index, take_violation, TxError, TxOutcome,
    };
}
