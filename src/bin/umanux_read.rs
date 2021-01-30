extern crate clap;
use std::cmp::Ordering;

use clap::{App, Arg};

extern crate umanux;
use umanux::{api::GroupRead, api::UserDBRead, api::UserRead, User, UserLibError};

fn main() -> Result<(), UserLibError> {
    env_logger::init();
    let matches = App::new("Create a new linux user")
        .version("0.1.0")
        .author("Franz Dietrich <dietrich@teilgedanken.de>")
        .about("Create a linux user do not use this in production (yet)")
        .arg(
            Arg::new("username")
                .value_name("USERNAME")
                .about("the new users name")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::new("passwd")
                .short('p')
                .long("passwd")
                .value_name("FILE")
                .about("The passwd file")
                .default_value("/etc/passwd")
                .takes_value(true),
        )
        .arg(
            Arg::new("shadow")
                .short('s')
                .long("shadow")
                .value_name("FILE")
                .about("The shadow file")
                .default_value("/etc/shadow")
                .takes_value(true),
        )
        .arg(
            Arg::new("group")
                .short('g')
                .long("group")
                .value_name("FILE")
                .about("The group file")
                .default_value("/etc/group")
                .takes_value(true),
        )
        .get_matches();

    let mf = umanux::Files::new(
        matches.value_of("passwd").unwrap(),
        matches.value_of("shadow").unwrap(),
        matches.value_of("group").unwrap(),
    )?;

    let db = umanux::UserDBLocal::load_files(mf).unwrap();

    let user = db
        .get_user_by_name(matches.value_of("username").unwrap())
        .expect("User not found");
    println!("{}", display_user(user));
    Ok(())
}

fn display_user(user: &User) -> String {
    // order the groups first by Membership kind, then by groupname
    let mut ordered_groups = user.get_groups().clone();
    ordered_groups.sort_by(|a, b| {
        let memord = a.0.cmp(&b.0);
        match memord {
            Ordering::Equal => {
                a.1.borrow()
                    .get_groupname()
                    .unwrap()
                    .cmp(b.1.borrow().get_groupname().unwrap())
            }
            other => other,
        }
    });

    format!(
        "Username: {}
Encrypted password: {}
Last Change: {}
UID: {}
Main GID: {}
Groups: \n{}",
        user.get_username().unwrap(),
        user.get_password().unwrap(),
        user.get_shadow()
            .unwrap()
            .get_last_change()
            .unwrap()
            .date()
            .format("%d.%m.%Y"),
        user.get_uid(),
        user.get_gid(),
        ordered_groups
            .iter()
            .map(|(mem, group)| {
                format!(
                    "  * {:#?}: {}",
                    mem,
                    group.borrow().get_groupname().unwrap()
                )
            })
            .collect::<Vec<String>>()
            .join(", \n"),
    )
}
