#![allow(dead_code)] // Allow dead code as some functions are called by the integration tests but not otherwise so they would trigger warnings.

use std::{env, path::Path};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

use std::ops::Deref;

#[derive(Debug)]
pub enum Source {
    Text(String),
    Path(PathBuf),
}

#[derive(Debug)]
pub struct Fixture {
    pub path: PathBuf,
    source: Source,
    _tempdir: TempDir,
}

impl Fixture {
    #[must_use]
    pub fn blank(fixture_filename: &str) -> Self {
        Self::build(fixture_filename, Source::Text("".to_owned()))
    }
    #[must_use]
    pub fn build(fixture_filename: &str, source: Source) -> Self {
        // The "real" path of the file is going to be under a temporary directory:
        let tempdir = tempfile::tempdir().expect("Failed to create a tmp dir.");
        let mut path = PathBuf::from(tempdir.path());
        path.push(&fixture_filename);

        {
            match source {
                Source::Path(ref p) => {
                    fs::copy(p, &path).unwrap();
                }
                Source::Text(ref c) => {
                    fs::write(&path, c).unwrap();
                }
            }
        }
        Self {
            _tempdir: tempdir,
            source,
            path,
        }
    }

    #[must_use]
    pub fn copy(fixture_filename: &str) -> Self {
        let root_dir = &env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR");
        let mut source = PathBuf::from(root_dir);
        source.push("src/tests/fixtures");
        source.push(&fixture_filename);

        Self::build(fixture_filename, Source::Path(source))
    }

    #[must_use]
    pub fn from_string(fixture_filename: &str, content: &str) -> Self {
        Self::build(fixture_filename, Source::Text(content.to_owned()))
    }
}

impl Deref for Fixture {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}
