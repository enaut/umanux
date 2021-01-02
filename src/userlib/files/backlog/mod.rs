use crate::UserLibError;

use super::FileContents;

pub trait ExecutableAtom {
    fn execute(self, content: String) -> Result<String, UserLibError>;
}

pub trait ExecutableUnit {
    fn execute(self, files: FileContents) -> Result<(), UserLibError>;
}

pub mod actions;
pub mod atoms;
