mod file;
mod mock;
mod objects;
mod raw;
pub use bincode;

#[doc(hidden)] // used by the `serialize_using_bincode!` macro
pub mod _reexports {
    pub use crate::raw::BoxedError;
}

pub use self::raw::{ObjectStore, ObjectStoreError, ObjectStoreFactory};
