//! A collection of small operations on the `/etc/{passwd,shadow,group}` files.
//!
//! No special checks are made. Usually one single such action does not make sense and should only be used in combination with others.
//! All operations implement [`super::ExecutableUnit`]. The structs in this module are only visible to [oplog](`super`).

use std::{cell::RefCell, rc::Rc};

use crate::{Group, User, UserLibError};

use super::ExecutableAtom;

pub(super) struct AddPasswdLine(pub Rc<User>);
impl ExecutableAtom for AddPasswdLine {
    fn execute(self, mut content: String) -> Result<String, UserLibError> {
        let selfline = self.0.to_string();
        content.push_str(&selfline);
        content.push('\n');
        Ok(content)
    }
}

#[test]
fn test_add_passwd_line() {
    let content = String::new();
    let first_user = Rc::new(crate::User::default());
    let apl = AddPasswdLine(Rc::clone(&first_user));

    // test first user adding
    let result_first = apl.execute(content).unwrap();
    // verify the number of lines
    assert!(result_first.lines().count() == 1);
    // verify the line content
    assert_eq!(result_first.trim_end(), &first_user.to_string());
    assert_eq!(
        result_first.trim_end(),
        "defaultusername:x:1001:1001::/:/bin/nologin"
    );

    // add a second user
    let apl2 = AddPasswdLine(Rc::clone(&first_user));
    let result_second = apl2.execute(result_first).unwrap();
    // verify the number of lines
    assert!(result_second.lines().count() == 2);
    // verify the line content
    for line in result_second.lines() {
        assert_eq!(line.trim_end(), &first_user.to_string());
    }

    // add a third and different user
    let second_user = Rc::new(crate::User::default().username("hänno".to_string()).clone());
    let apl3 = AddPasswdLine(Rc::clone(&second_user));
    let result_third = apl3.execute(result_second).unwrap();
    // verify the number of lines
    assert!(result_third.lines().count() == 3);
    // verify the line content
    assert_eq!(result_third.lines().next().unwrap(), first_user.to_string());
    assert_eq!(
        result_third.lines().last().unwrap(),
        second_user.to_string()
    );
}

pub(super) struct AddShadowLine(pub Rc<User>);
impl ExecutableAtom for AddShadowLine {
    fn execute(self, mut content: String) -> Result<String, UserLibError> {
        let selfline = self
            .0
            .get_shadow()
            .expect("The user does not have a shadow field but this is required")
            .to_string();
        content.push_str(&selfline);
        content.push('\n');
        Ok(content)
    }
}
#[test]
fn test_add_shadow_line() {
    let content = String::new();
    let first_user = Rc::new(crate::User::default());
    let apl = AddShadowLine(Rc::clone(&first_user));

    // test first user adding
    let result_first = apl.execute(content).unwrap();
    // verify the number of lines
    assert!(result_first.lines().count() == 1);
    // verify the line content
    assert_eq!(
        result_first.trim_end(),
        &first_user.get_shadow().unwrap().to_string()
    );
    assert_eq!(result_first.trim_end(), "defaultusername:!!:0:0:99999:7:::");

    // add a second user
    let apl2 = AddShadowLine(Rc::clone(&first_user));
    let result_second = apl2.execute(result_first).unwrap();
    // verify the number of lines
    assert!(result_second.lines().count() == 2);
    // verify the line content
    for line in result_second.lines() {
        assert_eq!(
            line.trim_end(),
            &first_user.get_shadow().unwrap().to_string()
        );
    }

    // add a third and different user
    let second_user = Rc::new(crate::User::default().username("hänno".to_string()).clone());
    let apl3 = AddShadowLine(Rc::clone(&second_user));
    let result_third = apl3.execute(result_second).unwrap();
    // verify the number of lines
    assert!(result_third.lines().count() == 3);
    // verify the line content
    assert_eq!(
        result_third.lines().next().unwrap(),
        first_user.get_shadow().unwrap().to_string()
    );
    assert_eq!(
        result_third.lines().last().unwrap(),
        second_user.get_shadow().unwrap().to_string()
    );
}

pub(super) struct AddGroupLine(pub Rc<RefCell<Group>>);
impl ExecutableAtom for AddGroupLine {
    fn execute(self, mut content: String) -> Result<String, UserLibError> {
        let selfline = self.0.borrow().to_string();
        content.push_str(&selfline);
        content.push('\n');
        Ok(content)
    }
}
#[test]
fn test_add_group_line() {
    let content = String::new();
    let line = "teste:x:1002:test,teste";
    let group = Rc::new(RefCell::new(line.parse().unwrap()));
    let apl = AddGroupLine(Rc::clone(&group));

    // test first user adding
    let result_first = apl.execute(content).unwrap();
    // verify the number of lines
    assert!(result_first.lines().count() == 1);
    // verify the line content
    assert_eq!(result_first.trim_end(), group.borrow().to_string());
    assert_eq!(result_first.trim_end(), "teste:x:1002:test,teste");

    // add a second user
    let apl2 = AddGroupLine(Rc::clone(&group));
    let result_second = apl2.execute(result_first).unwrap();
    // verify the number of lines
    assert!(result_second.lines().count() == 2);
    // verify the line content
    for line in result_second.lines() {
        assert_eq!(line.trim_end(), group.borrow().to_string());
    }

    // add a third and different user
    let second_user = Rc::new(RefCell::new("haenno:x:1002:test,teste".parse().unwrap()));
    let apl3 = AddGroupLine(Rc::clone(&second_user));
    let result_third = apl3.execute(result_second).unwrap();
    // verify the number of lines
    assert!(result_third.lines().count() == 3);
    // verify the line content
    assert_eq!(
        result_third.lines().next().unwrap(),
        group.borrow().to_string()
    );
    assert_eq!(
        result_third.lines().last().unwrap(),
        second_user.borrow().to_string()
    );
}

pub(super) struct DeletePasswdLine(Rc<User>);
impl ExecutableAtom for DeletePasswdLine {
    fn execute(self, content: String) -> Result<String, UserLibError> {
        let selfline = self.0.to_string();
        let lines = content.lines();
        let len_before = lines.count();
        let result = content
            .lines()
            .filter(|l| l != &selfline)
            .collect::<Vec<&str>>()
            .join("\n");
        let lenafter = result.lines().count();
        if len_before - lenafter == 1 {
            Ok(result)
        } else {
            Err("Failed to delete the user".into())
        }
    }
}
#[test]
fn test_delete_passwd_line() {
    let content = String::new();
    let first_user = Rc::new(crate::User::default());
    let second_user = Rc::new(crate::User::default().username("hänno".to_string()).clone());

    // Add a user and delete again
    let add_password_line = AddPasswdLine(Rc::clone(&first_user));
    let result_first = add_password_line.execute(content).unwrap();
    let delete_password_line = DeletePasswdLine(Rc::clone(&first_user));
    let result_first = delete_password_line.execute(result_first).unwrap();
    // verify the number of lines
    assert!(result_first.lines().count() == 0);
    // verify the line content
    assert_eq!(result_first, "");

    // delete from within other users
    let content = "defaultusername:x:1001:1001::/:/bin/nologin
defaultusername:x:1001:1001::/:/bin/nologin
hänno:x:1001:1001::/:/bin/nologin
defaultusername:x:1001:1001::/:/bin/nologin"
        .to_string();
    let delete_password_line = DeletePasswdLine(Rc::clone(&second_user));
    let result_second = delete_password_line.execute(content).unwrap();
    // verify the number of lines
    assert_eq!(result_second.lines().count(), 3);
    // verify the line content
    for line in result_second.lines() {
        assert_eq!(line.trim_end(), &first_user.to_string());
    }
    let delete_password_line = DeletePasswdLine(Rc::clone(&second_user));
    let result_third = delete_password_line.execute(result_second);
    assert_eq!(result_third, Err("Failed to delete the user".into()))
}

pub(super) struct DeleteShadowLine(Rc<User>);
impl ExecutableAtom for DeleteShadowLine {
    fn execute(self, content: String) -> Result<String, UserLibError> {
        let selfline = self
            .0
            .get_shadow()
            .expect("the user has to have a shadow entry")
            .to_string();
        let lines = content.lines();
        let len_before = lines.count();
        println!("{}", content);

        let result = content
            .lines()
            .filter(|l| l != &selfline)
            .collect::<Vec<&str>>()
            .join("\n");

        let lenafter = result.lines().count();
        if len_before - lenafter == 1 {
            Ok(result)
        } else {
            Err("Failed to delete the users shadow".into())
        }
    }
}
#[test]
fn test_delete_shadow_line() {
    let content = String::new();
    let first_user = Rc::new(crate::User::default());
    let second_user = Rc::new(crate::User::default().username("hänno".to_string()).clone());

    // Add a user and delete again
    let add_shadow_line = AddShadowLine(Rc::clone(&first_user));
    let result_first = add_shadow_line.execute(content).unwrap();
    let delete_shadow_line = DeleteShadowLine(Rc::clone(&first_user));
    let result_first = delete_shadow_line.execute(result_first).unwrap();
    // verify the number of lines
    assert!(result_first.lines().count() == 0);
    // verify the line content
    assert_eq!(result_first, "");

    // delete from within other users
    let content = "defaultusername:!!:0:0:99999:7:::
defaultusername:!!:0:0:99999:7:::
hänno:!!:0:0:99999:7:::
defaultusername:!!:0:0:99999:7:::"
        .to_string();
    let delete_password_line = DeleteShadowLine(Rc::clone(&second_user));
    let result_second = delete_password_line.execute(content).unwrap();
    // verify the number of lines
    assert_eq!(result_second.lines().count(), 3);
    // verify the line content
    for line in result_second.lines() {
        assert_eq!(
            line.trim_end(),
            &first_user.get_shadow().unwrap().to_string()
        );
    }
    let delete_password_line = DeleteShadowLine(Rc::clone(&second_user));
    let result_third = delete_password_line.execute(result_second);
    assert_eq!(
        result_third,
        Err("Failed to delete the users shadow".into())
    )
}

pub(super) struct DeleteGroupLine(Rc<RefCell<Group>>);
impl ExecutableAtom for DeleteGroupLine {
    fn execute(self, content: String) -> Result<String, UserLibError> {
        let selfline = self.0.borrow().to_string();
        let lines = content.lines();
        let len_before = lines.count();

        let result = content
            .lines()
            .filter(|l| l != &selfline)
            .collect::<Vec<&str>>()
            .join("\n");

        let lenafter = result.lines().count();
        if len_before - lenafter == 1 {
            Ok(result)
        } else {
            Err("Failed to delete the group".into())
        }
    }
}
#[test]
fn test_delete_group_line() {
    let content = String::new();
    let line = "teste:x:1002:test,teste";
    let group = Rc::new(RefCell::new(line.parse().unwrap()));
    let add_group_line = AddGroupLine(Rc::clone(&group));
    let delete_group_line = DeleteGroupLine(Rc::clone(&group));

    // test first user adding
    let result_first = add_group_line.execute(content).unwrap();
    let result_first = delete_group_line.execute(result_first).unwrap();
    // verify the number of lines
    assert!(result_first.lines().count() == 0);
    // verify the line content
    assert_eq!(result_first, "");

    // delete from within other users
    let content = "anders:x:1002:test,teste
anders:x:1002:test,teste
teste:x:1002:test,teste
anders:x:1002:test,teste"
        .to_string();
    let delete_password_line = DeleteGroupLine(Rc::clone(&group));
    let result_second = delete_password_line.execute(content).unwrap();
    // verify the number of lines
    assert_eq!(result_second.lines().count(), 3);
    // verify the line content
    for line in result_second.lines() {
        assert_eq!(line.trim_end(), "anders:x:1002:test,teste");
    }
    let delete_password_line = DeleteGroupLine(Rc::clone(&group));
    let result_third = delete_password_line.execute(result_second);
    assert_eq!(result_third, Err("Failed to delete the group".into()))
}
