use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    ops::Deref,
    path::{Path, PathBuf},
    rc::Rc,
};

use difference::Difference;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use crate::UserLibError;

use super::file_to_string;

#[derive(Debug, Clone)]
struct ChangeTrackingPath {
    old_content: OldContent,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct OldContent(RefCell<String>);

impl OldContent {
    #[must_use]
    pub const fn new(content: String) -> Self {
        Self(RefCell::new(content))
    }
}

impl ChangeTrackingPath {
    /// Lock the file corresponding with `path` read its contents and store it to check on later modifications if it is dirty.
    pub fn new(path: &str) -> Result<Self, UserLibError> {
        trace!("Creating changetracking path: {}", path);
        let mut lck = Files::try_to_lock_file(Path::new(path))?;

        let mut original_buf = String::new();
        lck.opened_file.read_to_string(&mut original_buf)?;
        lck.opened_file.seek(SeekFrom::Start(0))?;

        info!("Manually removing lock on {:?}", lck.lockpath);
        std::fs::remove_file(lck.lockpath).unwrap();

        Ok(Self {
            old_content: OldContent::new(original_buf.trim().to_owned()),
            path: Some(lck.filepath),
        })
    }
}

struct LockedFileResult {
    lockpath: PathBuf,
    filepath: PathBuf,
    opened_file: File,
}

#[derive(Debug, Clone)]
pub struct Files {
    passwd: Rc<ChangeTrackingPath>,
    shadow: Rc<ChangeTrackingPath>,
    group: Rc<ChangeTrackingPath>,
}

impl Files {
    /// use the default Linux `/etc/` paths
    pub fn default() -> Result<Self, UserLibError> {
        Ok(Self {
            passwd: Rc::new(ChangeTrackingPath::new("/etc/passwd")?),
            shadow: Rc::new(ChangeTrackingPath::new("/etc/shadow")?),
            group: Rc::new(ChangeTrackingPath::new("/etc/group")?),
        })
    }

    pub fn new(
        passwd_path: &str,
        shadow_path: &str,
        group_path: &str,
    ) -> Result<Self, UserLibError> {
        Ok(Self {
            passwd: Rc::new(ChangeTrackingPath::new(passwd_path)?),
            shadow: Rc::new(ChangeTrackingPath::new(shadow_path)?),
            group: Rc::new(ChangeTrackingPath::new(group_path)?),
        })
    }
    /// Check if all the files are defined. Because some operations require the files to be present
    #[must_use]
    pub fn is_virtual(&self) -> bool {
        !(self.group.path.is_some() & self.passwd.path.is_some() & self.shadow.path.is_some())
    }

    pub fn lock_all_get(
        &self,
    ) -> Result<(LockedFileGuard, LockedFileGuard, LockedFileGuard), UserLibError> {
        if self.passwd.path.is_some() && self.shadow.path.is_some() && self.group.path.is_some() {
            let pwd = self.lock_guarded_passwd()?;
            let shd = self.lock_guarded_shadow()?;
            let grp = self.lock_guarded_group()?;
            Ok((pwd, shd, grp))
        } else {
            Err(crate::UserLibError::FilesRequired)
        }
    }

    /// This function tries to lock a file in the way other passwd locking mechanisms work.
    ///
    /// * get the pid
    /// * create the temporary lockfilepath "/etc/passwd.12397"
    /// * create the lockfilepath "/etc/passwd.lock"
    /// * open the temporary file
    /// * write the pid to the tempfile
    /// * try to make a link from the temporary file created to the lockfile
    /// * ensure that the file has been linked successfully
    ///
    /// when the link could not be created:
    ///
    /// * Open the lockfile
    /// * read the contents of the lockfile
    /// * check if the lockfile contains a pid if not error out
    /// * check if the containing pid is in a valid format. If not create a matching error
    ///
    /// not implemented yet:
    ///
    /// * test if this process could be killed. If so disclose the pid in the error.
    /// * try to delete the lockfile as it is apparently not used by the process anmore. (cleanup)
    /// * try to lock again now that the old logfile has been safely removed.
    /// * remove the original file and only keep the lock hardlink
    fn try_to_lock_file(path: &Path) -> Result<LockedFileResult, UserLibError> {
        info!("locking file {}", path.to_string_lossy());
        let mut tempfilepath_const = path.to_owned();
        // get the pid
        let pid = std::process::id();
        debug!("using pid {}", std::process::id());
        // get the filename
        let filename = tempfilepath_const.file_name().unwrap().to_owned();
        // and the base path which is the base for tempfile and lockfile.
        tempfilepath_const.pop();
        let mut lockfilepath = tempfilepath_const.clone();
        // push the filenames to the paths
        tempfilepath_const.push(format!("{}.{}", filename.to_str().unwrap(), pid));
        let tempfilepath = TempLockFile {
            tlf: tempfilepath_const,
        };
        lockfilepath.push(format!("{}.lock", filename.to_str().unwrap()));
        debug!(
            "Lockfile paths: {:?} (temporary) {:?} (final)",
            *tempfilepath, lockfilepath
        );
        // write the pid into the tempfile
        {
            let mut tempfile = File::create(&*tempfilepath).unwrap_or_else(|e| {
                panic!("Failed to open {} error: {}", filename.to_str().unwrap(), e)
            });
            trace!("Writing {} into {}", pid, tempfilepath.to_string_lossy());
            write!(tempfile, "{}", pid).or_else(|e| {
                let error_msg = format!(
                    "could not write to {} error {}",
                    filename.to_string_lossy(),
                    e
                );
                error!("{}", error_msg);
                let err: crate::UserLibError = error_msg.into();
                Err(err)
            })?;
        }

        // try to make a hardlink from the lockfile to the tempfile
        let linkresult = std::fs::hard_link(&*tempfilepath, &lockfilepath);
        match linkresult {
            Ok(()) => {
                debug!("successfully locked");

                // open the file
                let resfile = OpenOptions::new().read(true).write(true).open(path);
                return match resfile {
                    Ok(file) => Ok(LockedFileResult {
                        lockpath: lockfilepath,
                        filepath: path.to_owned(),
                        opened_file: file,
                    }),
                    Err(e) => {
                        // failed to open the file undo the locks
                        let _ = std::fs::remove_file(&lockfilepath);
                        let ret: crate::UserLibError = format!(
                            "Failed to open the file: {}, error: {}",
                            path.to_string_lossy(),
                            e
                        )
                        .into();
                        Err(ret)
                    }
                };
            }
            Err(e) => match e.kind() {
                // analyze the error further
                std::io::ErrorKind::AlreadyExists => {
                    warn!("The file is already locked by another process! – testing the validity of the lock");
                    {
                        let mut lf = match File::open(&lockfilepath) {
                            Ok(file) => file,
                            Err(e) => {
                                panic!("failed to open the lockfile: {}", e);
                            }
                        };
                        let mut content = String::new();
                        lf.read_to_string(&mut content)
                            .unwrap_or_else(|e| panic!("failed to read the lockfile{}", e));

                        let content = content.trim().trim_matches(char::from(0));
                        let lock_pid = content.parse::<u32>();
                        match lock_pid {
                            Ok(pid) => {
                                warn!(
                                    "found a pid: {}, checking if this process is still running",
                                    pid
                                );
                                error!("The file could not be locked");
                                todo!("Validate the lock and delete the file if the process does not exist anymore");
                                /*let sent = nix::sys::signal::kill(
                                    nix::unistd::Pid::from_raw(pid as i32),
                                    nix::sys::signal::Signal::from(0),
                                );*/
                            }
                            Err(e) => error!(
                                "existing lock file {} with an invalid PID '{}' Error: {}",
                                lockfilepath.to_str().unwrap(),
                                content,
                                e
                            ),
                        }
                    }
                }

                _ => {
                    panic!("failed to lock the file: {}", e);
                }
            },
        }
        Err("was not able to lock!".into())
    }
    fn lock_guarded_passwd(&self) -> Result<LockedFileGuard, UserLibError> {
        let mut lck = Self::try_to_lock_file(self.passwd.path.as_ref().unwrap())?;
        let old_content = &*self.passwd.old_content.0.borrow();
        Self::check_if_dirty(old_content, &mut lck.opened_file)?;

        Ok(LockedFileGuard {
            lockfile: lck.lockpath,
            path: Rc::clone(&self.passwd),
            file: RefCell::new(lck.opened_file),
        })
    }
    fn lock_guarded_shadow(&self) -> Result<LockedFileGuard, UserLibError> {
        let mut lck = Self::try_to_lock_file(self.shadow.path.as_ref().unwrap())?;
        let old_content = &*self.shadow.old_content.0.borrow();
        Self::check_if_dirty(old_content, &mut lck.opened_file)?;
        Ok(LockedFileGuard {
            lockfile: lck.lockpath,
            path: Rc::clone(&self.shadow),
            file: RefCell::new(lck.opened_file),
        })
    }
    fn lock_guarded_group(&self) -> Result<LockedFileGuard, UserLibError> {
        let mut lck = Self::try_to_lock_file(self.group.path.as_ref().unwrap())?;
        let old_content = &*self.group.old_content.0.borrow();
        Self::check_if_dirty(old_content, &mut lck.opened_file)?;
        Ok(LockedFileGuard {
            lockfile: lck.lockpath,
            path: Rc::clone(&self.group),
            file: RefCell::new(lck.opened_file),
        })
    }

    fn check_if_dirty(original: &str, file: &mut File) -> Result<(), UserLibError> {
        let mut buf = String::new();
        file.seek(SeekFrom::Start(0))?;
        match file.read_to_string(&mut buf) {
            Ok(_) => {
                let buf = buf.trim().to_string();
                if original.trim().eq(&buf) {
                    file.seek(SeekFrom::Start(0))?;
                    Ok(())
                } else {
                    Err(
                        "The file has been modified by another process. Abort to avoid corruption."
                            .into(),
                    )
                }
            }
            Err(_) => Err("Could not proof an unchanged file prior to modification".into()),
        }
    }
}

#[derive(Debug)]
pub struct LockedFileGuard {
    lockfile: PathBuf,
    path: Rc<ChangeTrackingPath>,
    pub(crate) file: RefCell<File>,
}

#[derive(Debug)]
struct TempLockFile {
    tlf: PathBuf,
}

impl Drop for TempLockFile {
    fn drop(&mut self) {
        info!("removing temporary lockfile {}", self.tlf.to_str().unwrap());
        std::fs::remove_file(&self.tlf).unwrap();
    }
}

impl Deref for TempLockFile {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.tlf
    }
}

impl LockedFileGuard {
    pub fn print_difference(&self) -> Result<bool, UserLibError> {
        self.file.borrow_mut().seek(SeekFrom::Start(0))?;
        let new_content = file_to_string(&self.file.borrow_mut())?;
        let diffs =
            difference::Changeset::new(&self.path.old_content.0.borrow(), &new_content, "\n");
        let filtered = diffs
            .diffs
            .iter()
            .filter(|v| match v {
                Difference::Same(_) => false,
                Difference::Add(_) | Difference::Rem(_) => true,
            })
            .collect::<Vec<&Difference>>();
        println!(
            "\n\nPrinting the difference for {} \n\t{:?}",
            self.lockfile.to_string_lossy(),
            filtered
        );
        Ok(filtered.len() == 1)
    }
    pub fn replace_contents(&mut self, new_content: &str) -> Result<(), UserLibError> {
        // TODO: File read write permissions needed
        self.file = match OpenOptions::new()
            .truncate(true)
            .read(true)
            .write(true)
            .open(&self.path.path.as_ref().unwrap())
        {
            Ok(file) => RefCell::new(file),
            Err(e) => return Err(("Failed to truncate file.".to_owned(), e).into()),
        };
        match self
            .file
            .borrow_mut()
            .write_all(&new_content.to_owned().into_bytes())
        {
            Ok(_) => (),
            Err(e) => return Err(("Could not write (all) users. ".to_owned(), e).into()),
        };
        self.file.borrow_mut().write_all(b"\n")?;
        self.file.borrow_mut().flush()?;

        let mut s = self.path.old_content.0.borrow_mut();
        // update the new content as this is guaranteed to be correct.
        s.clear();
        s.push_str(new_content.trim());
        drop(s);
        Ok(())
    }

    pub fn append(&mut self, appendee: String) -> Result<(), UserLibError> {
        // Seek to the last character.
        self.file.borrow_mut().seek(SeekFrom::End(-1)).map_or_else(
            |e| Err(format!("Failed to append to file {}", e)),
            |_| Ok(()),
        )?;
        // Read the last character
        let mut b = [0_u8; 1];
        self.file.borrow_mut().read_exact(&mut b)?;
        // Verify it is '\n' else append '\n' so in any case the file ends with with a newline now
        if &b != b"\n" {
            //self.file.write_all(&b)?;
            self.file.borrow_mut().write_all(b"\n")?;
        }
        // write the new line.
        self.file
            .borrow_mut()
            .write_all(&appendee.into_bytes())
            .map_or_else(
                |e| Err(("Failed to append to file".to_owned(), e).into()),
                Ok,
            )
    }
}

impl Drop for LockedFileGuard {
    fn drop(&mut self) {
        info!("removing lock {:?}", self.lockfile);
        std::fs::remove_file(&self.lockfile).unwrap();
    }
}

#[test]
fn test_replace_a_file() -> Result<(), UserLibError> {
    use crate::Fixture;
    let pwds = Fixture::copy("passwds");
    let shds = Fixture::copy("shadows");
    let grps = Fixture::copy("groups");

    let fls = Files::new(
        &pwds.path.to_string_lossy(),
        &shds.path.to_string_lossy(),
        &grps.path.to_string_lossy(),
    )?;

    {
        let mut lpwd = fls.lock_guarded_passwd()?;
        lpwd.replace_contents(&"new_content".to_owned())?;
        // test that the cache is updated
        assert_eq!(*lpwd.path.old_content.0.borrow(), "new_content".to_owned());
        let mut desc = lpwd.file.borrow_mut();
        desc.seek(SeekFrom::Start(0))?;
        let cont = file_to_string(&*desc);
        let e = cont?;
        // test that the file contains the new data
        assert_eq!(e, "new_content\n");
    }
    let second_lpwd = fls.lock_guarded_passwd()?;
    assert_eq!(
        *second_lpwd.path.old_content.0.borrow(),
        "new_content".to_owned()
    );
    Ok(())
}

//#[test]
//fn test_replace_a_file() -> Result<(), UserLibError> {}
