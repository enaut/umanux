extern crate umanux;

use clap::{App, Arg};
use umanux::{api::UserDBWrite, UserLibError};
use umanux::{api::UserRead, userlib::Numbered};

extern crate env_logger;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

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
            Arg::new("remove_home")
                .short('r')
                .long("remove")
                .about("Also delete the home directory (default is to not delete it)")
                .takes_value(false)
                .required(false),
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

    let mut db = umanux::UserDBLocal::load_files(mf).unwrap();

    let user_res: Result<Numbered<umanux::User>, umanux::UserLibError> = db.delete_user(
        umanux::api::DeleteUserArgs::builder()
            .username(matches.value_of("username").unwrap())
            .delete_home(if matches.is_present("remove_home") {
                umanux::api::DeleteHome::Delete
            } else {
                umanux::api::DeleteHome::Keep
            })
            .build()
            .unwrap(),
    );
    match user_res {
        Ok(u) => {
            info!(
                "The user <{}> has been deleted! ",
                u.get_username().unwrap()
            );
            Ok(())
        }
        Err(e) => {
            error!("Failed to delete the user: {}", e);
            Err(e)
        }
    }
}
