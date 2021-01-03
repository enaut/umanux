use crate::{UserDBLocal, UserLibError};

use super::FileContents;

pub trait ExecutableAtom {
    /// A executable atom that adds, removes or modifies one line in file.
    fn execute(self, content: String) -> Result<String, UserLibError>;
}

pub trait ExecutableUnit {
    /// A executable Action that combines mutliple Atoms to a sensible activity.
    fn execute(self, contents: FileContents) -> Result<FileContents, UserLibError>;
}

pub trait ValidatableUnit {
    /// ValidatableUnits can validate the state to see if they are at all aplicable.
    fn validate(self, contents: FileContents, db: &UserDBLocal) -> Result<(), UserLibError>;
}

pub mod actions;
pub mod atoms;
