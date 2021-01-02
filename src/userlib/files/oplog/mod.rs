use crate::{UserDBLocal, UserLibError};

use super::FileContents;

pub trait ExecutableAtom {
    fn execute(self, content: String) -> Result<String, UserLibError>;
}

pub trait ExecutableUnit {
    fn execute(self, contents: FileContents) -> Result<FileContents, UserLibError>;
}

pub trait ValidatableUnit {
    fn validate(self, contents: FileContents, db: &UserDBLocal) -> Result<(), UserLibError>;
}

pub mod actions;
pub mod atoms;
