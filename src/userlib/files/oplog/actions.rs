//! Actions are collections of atoms to compose a usefull action to add, modify or delete a user.

use std::{cell::RefCell, rc::Rc};

use crate::{userlib::files::FileContents, Group, User};

use super::{
    atoms::{AddGroupLine, AddPasswdLine, AddShadowLine, DeleteGroupLine},
    ExecutableAtom, ExecutableUnit,
};

pub struct AddUserAction {
    pwd: AddPasswdLine,
    shd: AddShadowLine,
    grp: AddGroupLine,
}

impl ExecutableUnit for AddUserAction {
    fn execute(self, contents: FileContents) -> Result<FileContents, crate::UserLibError> {
        contents.pwd.replace(self.pwd.execute(contents.pwd.take())?);
        contents.shd.replace(self.shd.execute(contents.shd.take())?);
        contents.grp.replace(self.grp.execute(contents.grp.take())?);
        Ok(contents)
    }
}

impl AddUserAction {
    pub fn new(user: &Rc<User>, group: Rc<RefCell<Group>>) -> Self {
        Self {
            pwd: AddPasswdLine(Rc::clone(user)),
            shd: AddShadowLine(Rc::clone(user)),
            grp: AddGroupLine(Rc::clone(&group)),
        }
    }
}

pub struct AddGroupAction {
    grp: AddGroupLine,
}

impl ExecutableUnit for AddGroupAction {
    fn execute(self, contents: FileContents) -> Result<FileContents, crate::UserLibError> {
        contents.grp.replace(self.grp.execute(contents.grp.take())?);
        Ok(contents)
    }
}

pub struct DeleteGroupAction {
    grp: DeleteGroupLine,
}

impl ExecutableUnit for DeleteGroupAction {
    fn execute(self, contents: FileContents) -> Result<FileContents, crate::UserLibError> {
        contents.grp.replace(self.grp.execute(contents.grp.take())?);
        Ok(contents)
    }
}
