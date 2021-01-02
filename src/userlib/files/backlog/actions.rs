//! Actions are collections of atoms to compose a usefull action to add, modify or delete a user.

use crate::userlib::files::FileContents;

use super::{
    atoms::{AddGroupLine, AddPasswdLine, AddShadowLine},
    ExecutableAtom, ExecutableUnit,
};

struct AddUserAction {
    pwd: AddPasswdLine,
    shd: AddShadowLine,
    grp: AddGroupLine,
}

impl ExecutableUnit for AddUserAction {
    fn execute(self, contents: FileContents) -> Result<(), crate::UserLibError> {
        contents.pwd.replace(self.pwd.execute(contents.pwd.take())?);
        contents.shd.replace(self.shd.execute(contents.shd.take())?);
        contents.grp.replace(self.grp.execute(contents.grp.take())?);
        Ok(())
    }
}
