extern crate umanux;
mod testfiles;

#[test]
fn test_delete_user_function() {
    use testfiles::Fixture;

    use umanux::api::{GroupRead, UserDBRead, UserDBWrite, UserRead};

    let p = Fixture::copy("passwd");
    let s = Fixture::copy("shadow");
    let g = Fixture::copy("group");

    let pf = std::fs::read_to_string(&p.path).unwrap();

    let mf = umanux::Files::new(
        &p.path.to_string_lossy(),
        &s.path.to_string_lossy(),
        &g.path.to_string_lossy(),
    )
    .unwrap();

    let mut db = umanux::UserDBLocal::load_files(mf.clone()).unwrap();

    let user_res: Result<umanux::User, umanux::UserLibError> = db.delete_user(
        umanux::api::DeleteUserArgs::builder()
            .username("teste")
            // .delete_home(umanux::api::DeleteHome::Delete)
            .build()
            .unwrap(),
    );
    //assert_eq!(user_res, Err("".into()));
    let pf2 = std::fs::read_to_string(&p.path).unwrap();
    assert_eq!(user_res.unwrap().get_username().unwrap(), "teste");
    let pflines = pf.lines();
    let pflines2 = pf2.lines();
    for (l1, l2) in pflines.zip(pflines2.clone()) {
        if l1 != l2 {
            assert!(l1.starts_with("teste"));
            assert!(l2.starts_with("bergfried"));
            break;
        }
    }
    for line in pflines2 {
        assert!(!line.starts_with("teste"))
    }
    let groupfile2 = std::fs::read_to_string(&g.path).unwrap();
    let groupfilelines2 = groupfile2.lines();
    for line in groupfilelines2 {
        println!("{}", &line);
        assert!(!line.ends_with("teste"))
    }

    let user_res_test: Result<umanux::User, umanux::UserLibError> = db.delete_user(
        umanux::api::DeleteUserArgs::builder()
            .username("test")
            // .delete_home(umanux::api::DeleteHome::Delete)
            .build()
            .unwrap(),
    );
    println!("{:?}", user_res_test);
    if let Ok(u) = user_res_test {
        assert_eq!(u.get_username(), Some("test"))
    } else {
        panic!("The user was not deleted")
    }
    let mf = umanux::Files::new(
        &p.path.to_string_lossy(),
        &s.path.to_string_lossy(),
        &g.path.to_string_lossy(),
    )
    .unwrap();
    let parsed_again = umanux::UserDBLocal::load_files(mf.clone()).unwrap();
    let group = parsed_again
        .get_group_by_id(1002)
        .expect("this group should exist");
    assert_eq!(
        group
            .borrow()
            .get_member_names()
            .expect("should be empty list")
            .len(),
        0
    );
}
