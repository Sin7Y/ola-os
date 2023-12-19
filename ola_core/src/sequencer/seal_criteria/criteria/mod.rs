pub mod slots;

pub use slots::SlotsCriterion;
pub mod geometry_seal_criteria;
pub use geometry_seal_criteria::{InitialWritesCriterion, RepeatedWritesCriterion};