#![allow(clippy::non_ascii_literal)]

pub mod files;
pub mod hashes;

use crate::{
    api::{
        CreateUserArgs, DeleteHome, DeleteUserArgs, GroupRead, UserDBRead, UserDBWrite, UserRead,
    },
    group::MembershipKind,
    Group, Shadow, User, UserLibError,
};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    ops::{Deref, DerefMut},
    rc::Rc,
    str::FromStr,
};

pub type UserList = HashMap<String, Numbered<User>>;

pub struct UserDBLocal {
    source_files: Option<files::Files>,
    pub users: UserList,
    pub groups: Vec<Rc<RefCell<Numbered<Group>>>>,
}

impl UserDBLocal {
    /// Import the database from strings
    pub fn import_from_strings(
        passwd_content: &str,
        shadow_content: &str,
        group_content: &str,
    ) -> Result<Self, UserLibError> {
        let shadow_entries: Vec<Numbered<Shadow>> = string_to(shadow_content)?;
        let mut users = user_vec_to_hashmap(string_to(passwd_content)?);
        let groups: Vec<Rc<RefCell<Numbered<Group>>>> = string_to::<Group>(group_content)?
            .into_iter()
            .map(|x| Rc::new(RefCell::new(x)))
            .collect();
        shadow_to_users(&mut users, shadow_entries);
        groups_to_users(&mut users, &groups);
        Ok(Self {
            source_files: None,
            users,
            groups,
        })
    }

    /// Import the database from a [`crate::userlib::files::Files`] struct
    pub fn load_files(files: files::Files) -> Result<Self, crate::UserLibError> {
        // Get the Strings for the files use an inner block to drop references after read.
        let (my_passwd_lines, my_shadow_lines, my_group_lines) = {
            let opened = files.lock_all_get();
            let (locked_p, locked_s, locked_g) = opened.expect("failed to lock files!");
            // read the files to strings
            let p = file_to_string(&locked_p.file.borrow_mut())?;
            let s = file_to_string(&locked_s.file.borrow_mut())?;
            let g = file_to_string(&locked_g.file.borrow_mut())?;
            // return the strings to the outer scope and release the lock...
            (p, s, g)
        };

        let mut users = user_vec_to_hashmap(string_to(&my_passwd_lines)?);
        let passwds: Vec<Numbered<Shadow>> = string_to(&my_shadow_lines)?;
        let groups: Vec<Rc<RefCell<Numbered<Group>>>> = string_to::<Group>(&my_group_lines)?
            .into_iter()
            .map(|x| Rc::new(RefCell::new(x)))
            .collect();
        shadow_to_users(&mut users, passwds);
        groups_to_users(&mut users, &groups);
        Ok(Self {
            source_files: Some(files),
            users,
            groups,
        })
    }
    fn delete_from_passwd(
        user: &User,
        locked_p: &mut files::LockedFileGuard,
    ) -> Result<(), UserLibError> {
        let passwd_file_content = file_to_string(&locked_p.file.borrow_mut())?;
        let modified_p = user.remove_in(&passwd_file_content);

        // write the new content to the file.
        let ncont = locked_p.replace_contents(&modified_p);
        match ncont {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to write the passwd database: {}", e).into()),
        }
    }

    fn delete_from_shadow(
        user: &User,
        locked_s: &mut files::LockedFileGuard,
    ) -> Result<(), UserLibError> {
        let shad = user.get_shadow();
        let shadow_file_content = file_to_string(&locked_s.file.borrow_mut())?;
        match shad {
            Some(shadow) => {
                let modified_s = shadow.remove_in(&shadow_file_content);
                let ncont = locked_s.replace_contents(&modified_s);
                match ncont {
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!(
                        "Error during write to the shadow database. \
                    Please doublecheck as the shadowdatabase could be corrupted: {}",
                        e,
                    )
                    .into()),
                }
            }
            None => Ok(()),
        }
    }

    fn delete_from_group(
        group: &Rc<RefCell<Numbered<Group>>>,
        locked_g: &mut files::LockedFileGuard,
    ) -> Result<(), UserLibError> {
        let group_file_content = file_to_string(&locked_g.file.borrow_mut())?;
        let modified_g = group.borrow().remove_in(&group_file_content);

        let replace_result = locked_g.replace_contents(&modified_g);
        match replace_result {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "Error during write to the database. \
            Please doublecheck as the groupdatabase could be corrupted: {}",
                e,
            )
            .into()),
        }
    }

    fn write_groups(&self, locked_g: &mut files::LockedFileGuard) -> Result<(), UserLibError> {
        let content = self
            .groups
            .iter()
            .map(|g| (g.borrow().to_string()))
            .collect::<Vec<String>>()
            .join("\n");
        let replace_result = locked_g.replace_contents(&content);
        match replace_result {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "Error during write to the database. \
            Please doublecheck as the groupdatabase could be corrupted: {}",
                e,
            )
            .into()),
        }
    }

    fn delete_home(user: &User) -> std::io::Result<()> {
        if let Some(dir) = user.get_home_dir() {
            std::fs::remove_dir_all(dir)
        } else {
            let error_msg = "Failed to remove the home directory! As the user did not have one.";
            error!("{}", error_msg);
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                error_msg,
            ))
        }
    }

    pub fn delete_group_by_id(&mut self, gid: u32) {
        self.groups
            .retain(|g| g.borrow().get_gid().expect("groups have to have a gid") != gid);
    }
}

impl UserDBWrite for UserDBLocal {
    fn delete_user(&mut self, args: DeleteUserArgs) -> Result<Numbered<User>, UserLibError> {
        // try to get the user from the database
        let user_opt = self.get_user_by_name(args.username);
        let user = match user_opt {
            Some(user) => user.clone(),
            None => {
                return Err(UserLibError::NotFound);
            }
        };

        if self.source_files.is_none() {
            warn!("There are no associated files working in dummy mode!");
            let res = self.users.remove(args.username);
            match res {
                Some(u) => Ok(u),
                None => Err(UserLibError::NotFound), // should not happen anymore as existence is checked.
            }
        } else {
            let (mut locked_p, mut locked_s, mut locked_g) = {
                let opened = self.source_files.as_ref().unwrap().lock_all_get();
                opened.expect("failed to lock files!")
            };

            Self::delete_from_passwd(&user, &mut locked_p)?;
            //locked_p.print_difference()?;
            Self::delete_from_shadow(&user, &mut locked_s)?;
            if args.delete_home == DeleteHome::Delete {
                Self::delete_home(&user)?;
            }
            //locked_p.print_difference()?;
            trace!("The users groups: {:#?}", user.get_groups());
            // Iterate over the GIDs to avoid borrowing issues
            let users_groups: Vec<(MembershipKind, u32)> = user
                .get_groups()
                .iter()
                .map(|(k, g)| (*k, g.borrow().get_gid().unwrap()))
                .collect();
            for (kind, group) in users_groups {
                trace!("Woring on group: {:?} - {}", kind, group);
                match kind {
                    crate::group::MembershipKind::Primary => {
                        if self
                            .get_group_by_id(group)
                            .expect("The group does not exist")
                            .borrow()
                            .get_member_names()
                            .expect("this group allways has a member")
                            .len()
                            == 1
                        {
                            trace!(
                                "Deleting group as the user to be deleted is the only member {}",
                                self.get_group_by_id(group)
                                    .expect("The group does not exist")
                                    .borrow()
                                    .get_groupname()
                                    .expect("a group has to have a name")
                            );
                            Self::delete_from_group(
                                &self
                                    .get_group_by_id(group)
                                    .expect("The group does not exist"),
                                &mut locked_g,
                            )?;
                            // remove the group from the groups Vec
                            self.groups.retain(|g| {
                                g.borrow().get_gid().expect("groups have to have a gid") != group
                            });
                        } else {
                            // remove the membership from the group instead of deleting the group if he was not the only user in its primary group.
                            if let Some(group) = self.get_group_by_id(group) {
                                group
                                    .borrow_mut()
                                    .remove_member(MembershipKind::Primary, args.username)
                            };
                            self.write_groups(&mut locked_g)?;
                            warn!(
                                    "The primary group (GID: {}) was not empty and is thus not removed. Only the membership has been removed",
                                    group
                                );
                        }
                    }
                    crate::group::MembershipKind::Member => {
                        trace!("delete the membership in the group");
                        if let Some(group) = self.get_group_by_id(group) {
                            group
                                .borrow_mut()
                                .remove_member(MembershipKind::Member, args.username);
                            trace!("The new group: {:?}", group.borrow());
                        };
                        self.write_groups(&mut locked_g)?;
                    }
                }
            }
            // Remove the user from the memory database(HashMap)
            let res = self.users.remove(args.username);
            match res {
                Some(u) => Ok(u),
                None => Err("Failed to remove the user from the internal HashMap".into()),
            }
        }
    }

    fn new_user(&mut self, args: CreateUserArgs) -> Result<&Numbered<User>, crate::UserLibError> {
        if self.users.contains_key(args.username) {
            Err(format!("The username {} already exists! Aborting!", args.username).into())
        } else {
            let mut new_user = User::default();
            new_user.username(args.username.to_owned());
            if self.users.contains_key(args.username) {
                Err("Failed to create the user. A user with the same Name already exists".into())
            } else {
                if self.source_files.is_some() {
                    let opened = self.source_files.as_ref().unwrap().lock_all_get();
                    let (mut locked_p, mut locked_s, mut _locked_g) =
                        opened.expect("failed to lock files!");
                    //dbg!(&locked_p);
                    locked_p.append(format!("{}", new_user))?;
                    if let Some(shadow) = new_user.get_shadow() {
                        info!("Adding shadow entry {}", shadow);
                        locked_s.append(format!("{}", shadow))?;
                    } else {
                        warn!("Omitting shadow entry!")
                    }
                } else {
                    warn!("Working without database files this change cannot be stored.")
                }
                assert!(self
                    .users
                    .insert(
                        args.username.to_owned(),
                        Numbered {
                            pos: usize::max_value(),
                            value: new_user
                        }
                    )
                    .is_none());
                self.users
                    .get(args.username)
                    .map_or_else(|| Err("User was not successfully added!".into()), Ok)
            }
        }
    }

    fn delete_group(&mut self, _group: Rc<RefCell<Group>>) -> Result<(), UserLibError> {
        todo!()
    }

    fn new_group(&mut self) -> Result<Rc<RefCell<Group>>, UserLibError> {
        todo!()
    }
}

impl UserDBRead for UserDBLocal {
    fn get_all_users(&self) -> Vec<&User> {
        let mut res: Vec<&User> = self.users.iter().map(|(_, x)| &x.value).collect();
        res.sort();
        res
    }

    fn get_user_by_name(&self, name: &str) -> Option<&User> {
        self.users.get(name).map(|num| &num.value)
    }

    fn get_user_by_id(&self, uid: u32) -> Option<&User> {
        // could probably be more efficient - on the other hand its no problem to loop a thousand users.
        for user in self.users.values() {
            if user.get_uid() == uid {
                return Some(&user.value);
            }
        }
        None
    }

    fn get_all_groups(&self) -> Vec<Rc<RefCell<Numbered<Group>>>> {
        self.groups.iter().map(Clone::clone).collect()
    }

    fn get_group_by_name(&self, name: &str) -> Option<Rc<RefCell<Numbered<Group>>>> {
        for group in &self.groups {
            if group.borrow().get_groupname()? == name {
                return Some(Rc::clone(group));
            }
        }
        None
    }

    fn get_group_by_id(&self, id: u32) -> Option<Rc<RefCell<Numbered<Group>>>> {
        for group in &self.groups {
            if group.borrow().get_gid()? == id {
                return Some(Rc::clone(group));
            }
        }
        None
    }
}

use crate::api::UserDBValidation;
impl UserDBValidation for UserDBLocal {
    fn is_uid_valid_and_free(&self, uid: u32) -> bool {
        warn!("No valid check, only free check");
        let free = self.users.iter().all(|(_, u)| u.get_uid() != uid);
        free
    }

    fn is_username_valid_and_free(&self, name: &str) -> bool {
        let valid = crate::user::passwd_fields::is_username_valid(name);
        let free = self.get_user_by_name(name).is_none();
        valid && free
    }

    fn is_gid_valid_and_free(&self, gid: u32) -> bool {
        warn!("No valid check, only free check");
        self.groups
            .iter()
            .all(|x| x.borrow().get_gid().unwrap() != gid)
    }

    fn is_groupname_valid_and_free(&self, name: &str) -> bool {
        let valid = crate::group::is_groupname_valid(name);
        let free = self
            .groups
            .iter()
            .all(|x| x.borrow().get_groupname().unwrap() != name);
        valid && free
    }
}

/// Parse a file to a string
fn file_to_string(file: &File) -> Result<String, crate::UserLibError> {
    let mut reader = BufReader::new(file);
    let mut lines = String::new();
    let res = reader.read_to_string(&mut lines);
    match res {
        Ok(_) => Ok(lines),
        Err(e) => Err(format!("failed to read the file: {:?}", e).into()),
    }
}

fn groups_to_users<'a>(
    users: &'a mut UserList,
    groups: &[Rc<RefCell<Numbered<Group>>>],
) -> &'a mut UserList {
    // Populate the regular groups

    for group in groups.iter() {
        match group.borrow().get_member_names() {
            Some(usernames) => {
                for username in usernames {
                    if let Some(user) = users.get_mut(username) {
                        user.add_group(crate::group::MembershipKind::Member, group.clone());
                    }
                }
            }
            None => continue,
        }
    }

    // Populate the primary membership
    for user in users.values_mut() {
        let gid = user.get_gid();
        let grouplist: Vec<Rc<RefCell<Numbered<Group>>>> = groups
            .iter()
            .filter_map(|g| {
                if g.borrow().get_gid().unwrap() == gid {
                    Some(Rc::clone(g))
                } else {
                    None
                }
            })
            .collect();
        if grouplist.len() == 1 {
            let group = grouplist.first().unwrap();
            group.borrow_mut().append_user(
                user.get_username()
                    .expect("Users without username are not supported"),
            );
            user.add_group(crate::group::MembershipKind::Primary, group.clone());
        } else {
            error!(
                "Somehow the group with gid {} was found {} times",
                gid,
                grouplist.len()
            );
        }
    }
    users
}

/// Merge the Shadow passwords into the users
fn shadow_to_users(users: &mut UserList, shadow: Vec<Numbered<Shadow>>) -> &mut UserList {
    for pass in shadow {
        let user = users
            .get_mut(pass.get_username())
            .unwrap_or_else(|| panic!("the user {} does not exist", pass.get_username()));
        user.password = crate::Password::Shadow(pass);
    }
    users
}

/// Convert a `Vec<crate::User>` to a `UserList` (`HashMap<String, crate::User>`) where the username is used as key
fn user_vec_to_hashmap(users: Vec<Numbered<User>>) -> UserList {
    users
        .into_iter()
        .map(|x| {
            (
                x.get_username()
                    .expect("An empty username is not supported")
                    .to_owned(),
                x,
            )
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Numbered<T>
where
    T: Clone,
{
    pub pos: usize,
    pub value: T,
}

impl<T> Ord for Numbered<T>
where
    T: Eq + Clone,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.pos.cmp(&other.pos)
    }
}

impl<T> PartialOrd for Numbered<T>
where
    T: Eq + Clone,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.pos.partial_cmp(&other.pos)
    }
}

impl<T> Deref for Numbered<T>
where
    T: Clone,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Numbered<T>
where
    T: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

/// Try to parse a String into some Object.
///
/// # Errors
/// if the parsing failed a [`UserLibError::Message`](crate::error::UserLibError::Message) is returned containing a more detailed error message.
pub trait LineNumerable {
    fn set_line(&mut self, position: u32) -> &mut Self;
}

/// A generic function that parses a string line by line and creates the appropriate `Vec<T>` requested by the type system.
fn string_to<T>(source: &str) -> Result<Vec<Numbered<T>>, T::Err>
where
    T: FromStr + Clone,
{
    source
        .to_owned()
        .lines()
        .map(str::parse)
        .enumerate()
        .map(|(pos, item)| match item {
            Ok(value) => Ok(Numbered { pos, value }),
            Err(e) => Err(e),
        })
        .collect()
}

#[test]
fn test_creator_user_db_local() {
    let data = UserDBLocal::import_from_strings("test:x:1002:1002:full Name,004,000342,001-2312,myemail@test.com:/home/test:/bin/test", "test:$6$u0Hh.9WKRF1Aeu4g$XqoDyL6Re/4ZLNQCGAXlNacxCxbdigexEqzFzkOVPV5Z1H23hlenjW8ZLgq6GQtFURYwenIFpo1c.r4aW9l5S/:18260:0:99999:7:::", "teste:x:1002:\nanother:x:1003:test").unwrap();
    assert_eq!(
        data.users.get("test").unwrap().get_username().unwrap(),
        "test"
    );
    for user in data.users.values() {
        dbg!(user.get_groups());
        let (member_group1, group1) = user.get_groups().first().unwrap();
        let (member_group2, group2) = user.get_groups().get(1).unwrap();

        assert_eq!(*member_group1, crate::group::MembershipKind::Member);
        assert_eq!(group1.borrow().get_groupname(), Some("another"));
        assert_eq!(*member_group2, crate::group::MembershipKind::Primary);
        assert_eq!(group2.borrow().get_groupname(), Some("teste"));
    }
}

#[test]
fn test_parsing_local_database() {
    use std::path::PathBuf;
    // Parse the worldreadable user database ignore the shadow database as this would require root privileges.
    let pwdfile = File::open(PathBuf::from("/etc/passwd")).unwrap();
    let grpfile = File::open(PathBuf::from("/etc/group")).unwrap();
    let my_passwd_lines = file_to_string(&pwdfile).unwrap();
    let my_group_lines = file_to_string(&grpfile).unwrap();
    let data = UserDBLocal::import_from_strings(&my_passwd_lines, "", &my_group_lines).unwrap();
    assert_eq!(
        data.groups
            .get(0)
            .unwrap()
            .borrow()
            .get_groupname()
            .unwrap(),
        "root"
    );
}

#[test]
fn test_user_db_read_implementation() {
    use std::path::PathBuf;
    let pwdfile = File::open(PathBuf::from("/etc/passwd")).unwrap();
    let grpfile = File::open(PathBuf::from("/etc/group")).unwrap();
    let pass = file_to_string(&pwdfile).unwrap();
    let group = file_to_string(&grpfile).unwrap();
    let data = UserDBLocal::import_from_strings(&pass, "", &group).unwrap();
    // Usually there are more than 10 users
    assert!(data.get_all_users().len() > 10);
    assert!(data.get_user_by_name("root").is_some());
    assert_eq!(data.get_user_by_name("root").unwrap().get_uid(), 0);
    assert_eq!(
        data.get_user_by_id(0).unwrap().get_username().unwrap(),
        "root"
    );
    assert!(data.get_all_groups().len() > 10);
    assert!(data.get_group_by_name("root").is_some());
    assert_eq!(
        data.get_group_by_name("root")
            .unwrap()
            .borrow()
            .get_gid()
            .unwrap(),
        0
    );
    assert_eq!(
        data.get_group_by_id(0)
            .unwrap()
            .borrow()
            .get_groupname()
            .unwrap(),
        "root"
    );
    assert!(data.get_user_by_name("norealnameforsure").is_none());
    assert!(data.get_group_by_name("norealgroupforsure").is_none());
}

#[test]
fn test_user_db_write_implementation() {
    use crate::api::DeleteUserArgs;
    let mut data = UserDBLocal::import_from_strings("test:x:1001:1001:full Name,004,000342,001-2312,myemail@test.com:/home/test:/bin/test", "test:$6$u0Hh.9WKRF1Aeu4g$XqoDyL6Re/4ZLNQCGAXlNacxCxbdigexEqzFzkOVPV5Z1H23hlenjW8ZLgq6GQtFURYwenIFpo1c.r4aW9l5S/:18260:0:99999:7:::", "teste:x:1002:test,test").unwrap();
    let user = "test";

    assert_eq!(data.get_all_users().len(), 1);
    assert!(data
        .delete_user(DeleteUserArgs::builder().username(user).build().unwrap())
        .is_ok());
    assert!(data
        .delete_user(DeleteUserArgs::builder().username(user).build().unwrap())
        .is_err());
    assert_eq!(data.get_all_users().len(), 0);
}
