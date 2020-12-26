extern crate umanux;
use umanux::api::UserDBRead;
use umanux::{api::GroupRead, UserLibError};

fn main() -> Result<(), UserLibError> {
    simplelog::CombinedLogger::init(vec![simplelog::TermLogger::new(
        simplelog::LevelFilter::Warn,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
    )])
    .unwrap();

    let db = umanux::UserDBLocal::load_files(umanux::Files::default()?)?;

    for u in db.get_all_users() {
        println!("{}", u);
        println!(
            "Groups: {:?}",
            u.get_groups()
                .iter()
                .map(|group| {
                    (
                        format!("{:?}", group.0),
                        group.1.borrow().get_groupname().unwrap().to_owned(),
                    )
                })
                .collect::<Vec<(String, String)>>()
        );
    }

    let gr_res = db.get_all_groups();
    for group in gr_res {
        let gp = group.borrow();
        println!("{}", gp);
        println!("{:?}", gp.get_member_names())
    }
    Ok(())
}
