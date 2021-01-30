use crate::UserLibError;
use std::convert::TryFrom;
use std::fmt::{self, Debug, Display};
use std::{cmp::Eq, str::FromStr};

/// A record(line) in the user database `/etc/shadow` found in most linux systems.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Shadow {
    source: String,
    username: crate::Username,                      /* Username.  */
    pub(crate) password: crate::EncryptedPassword,  /* Hashed passphrase */
    last_change: Option<chrono::NaiveDateTime>,     /* User ID.  */
    earliest_change: Option<chrono::NaiveDateTime>, /* Group ID.  */
    latest_change: Option<chrono::NaiveDateTime>,   /* Real name.  */
    warn_period: Option<chrono::Duration>,          /* Home directory.  */
    deactivated: Option<chrono::Duration>,          /* Shell program.  */
    deactivated_since: Option<chrono::Duration>,    /* Shell program.  */
    extensions: Option<u64>,                        /* Shell program.  */
}

impl Shadow {
    #[must_use]
    pub fn get_username(&self) -> &str {
        &self.username.username
    }
    #[must_use]
    pub const fn get_last_change(&self) -> Option<&chrono::NaiveDateTime> {
        self.last_change.as_ref()
    }

    pub fn set_username(&mut self, username: crate::Username) {
        self.username = username;
    }

    #[must_use]
    pub fn get_password(&self) -> &str {
        &self.password.password
    }
    #[must_use]
    pub fn remove_in(&self, content: &str) -> String {
        content
            .split(&self.source)
            .map(str::trim)
            .collect::<Vec<&str>>()
            .join("\n")
    }
}

impl Display for Shadow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}:{}:{}:{}:{}:{}:{}:{}:{}",
            self.username,
            self.password,
            show_option_date(self.last_change),
            show_option_date(self.earliest_change),
            show_option_date(self.latest_change),
            show_option_duration(self.warn_period),
            show_option_duration(self.deactivated),
            show_option_duration(self.deactivated_since),
            if self.extensions.is_none() {
                "".to_string()
            } else {
                self.extensions.unwrap().to_string()
            }
        )
    }
}

fn show_option_date(input: Option<chrono::NaiveDateTime>) -> String {
    if input.is_none() {
        "".into()
    } else {
        format!("{}", input.unwrap().timestamp() / SECONDS_PER_DAY)
    }
}

fn show_option_duration(input: Option<chrono::Duration>) -> String {
    if input.is_none() {
        "".into()
    } else {
        format!("{}", input.unwrap().num_days())
    }
}

impl FromStr for Shadow {
    /// Parse a line formatted like one in `/etc/shadow` and construct a matching `Shadow` instance
    ///
    /// # Example
    /// ```
    /// use std::str::FromStr;
    /// let shad: umanux::Shadow = "test:$6$u0Hh.9WKRF1Aeu4g$XqoDyL6Re/4ZLNQCGAXlNacxCxbdigexEqzFzkOVPV5Z1H23hlenjW8ZLgq6GQtFURYwenIFpo1c.r4aW9l5S/:18260:0:99999:7:::".parse().unwrap();
    /// assert_eq!(shad.get_username(), "test");
    /// ```
    ///
    /// # Errors
    /// When parsing fails this function returns a `UserLibError::Message` containing some information as to why the function failed.
    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let elements: Vec<String> = line.split(':').map(ToString::to_string).collect();
        if elements.len() == 9 {
            let extra = elements.get(8).unwrap();
            Ok(Self {
                source: line.to_owned(),
                username: crate::Username::try_from(elements.get(0).unwrap().to_string())?,
                password: crate::EncryptedPassword::try_from(elements.get(1).unwrap().to_string())?,
                last_change: date_since_epoch(elements.get(2).unwrap()),
                earliest_change: date_since_epoch(elements.get(3).unwrap()),
                latest_change: date_since_epoch(elements.get(4).unwrap()),
                warn_period: duration_for_days(elements.get(5).unwrap()),
                deactivated: duration_for_days(elements.get(6).unwrap()),
                deactivated_since: duration_for_days(elements.get(7).unwrap()),
                extensions: if extra.is_empty() {
                    None
                } else {
                    Some(extra.parse::<u64>().unwrap())
                },
            })
        } else {
            Err(format!(
                "Failed to parse: not enough elements ({}): {:?}",
                elements.len(),
                elements
            )
            .into())
        }
    }

    type Err = UserLibError;
}

const SECONDS_PER_DAY: i64 = 86400;

fn date_since_epoch(days_since_epoch: &str) -> Option<chrono::NaiveDateTime> {
    if days_since_epoch.is_empty() {
        None
    } else {
        let days: i64 = days_since_epoch.parse::<i64>().unwrap();
        let seconds = days * SECONDS_PER_DAY;
        Some(chrono::NaiveDateTime::from_timestamp(seconds, 0))
    }
}
fn duration_for_days(days_source: &str) -> Option<chrono::Duration> {
    if days_source.is_empty() {
        None
    } else {
        let days: i64 = days_source.parse::<i64>().unwrap();
        Some(chrono::Duration::days(days))
    }
}

#[test]
fn test_parse_and_back_identity() {
    let line = "test:$6$u0Hh.9WKRF1Aeu4g$XqoDyL6Re/4ZLNQCGAXlNacxCxbdigexEqzFzkOVPV5Z1H23hlenjW8ZLgq6GQtFURYwenIFpo1c.r4aW9l5S/:18260:0:99999:7:::";
    let line2: Shadow = line.parse().unwrap();
    assert_eq!(format!("{}", line2), line);
}
