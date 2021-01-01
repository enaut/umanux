use crate::UserLibError;

pub trait ExecutableUnit {
    fn execute(self, content: String) -> Result<String, UserLibError>;
}

mod atoms;
