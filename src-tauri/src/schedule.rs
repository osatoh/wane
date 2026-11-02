//! Daily schedule stored as TOML (Spec §3).

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Default schedule written on first launch (Spec §3 "Default schedule").
pub const DEFAULT_SCHEDULE: &str = r#"[[entry]]
time = "07:00"
title = "Wake up"

[[entry]]
time = "07:30"
title = "Breakfast"

[[entry]]
time = "09:00"
title = "Start work"

[[entry]]
time = "12:00"
title = "Lunch"

[[entry]]
time = "13:00"
title = "Back to work"

[[entry]]
time = "18:00"
title = "Finish work"

[[entry]]
time = "19:00"
title = "Dinner"

[[entry]]
time = "21:00"
title = "Wind down"

[[entry]]
time = "23:00"
title = "Bed"
"#;

/// Minutes since local midnight, parsed from `HH:MM` (24-hour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeOfDay(u16);

impl TimeOfDay {
    pub fn minutes(self) -> u16 {
        self.0
    }

    fn parse(s: &str) -> Option<Self> {
        // Accept only two ASCII digits per field
        let b = s.as_bytes();
        if b.len() != 5 || b[2] != b':' {
            return None;
        }
        let two = |i: usize| -> Option<u16> {
            let (h, l) = (b[i], b[i + 1]);
            (h.is_ascii_digit() && l.is_ascii_digit())
                .then(|| u16::from((h - b'0') * 10 + (l - b'0')))
        };
        let (hh, mm) = (two(0)?, two(3)?);
        (hh < 24 && mm < 60).then_some(Self(hh * 60 + mm))
    }
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.0 / 60, self.0 % 60)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub time: TimeOfDay,
    pub title: String,
}

/// Validated schedule; entries are sorted by time and have unique times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    entries: Vec<Entry>,
}

impl Schedule {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

#[derive(Debug)]
pub enum ScheduleError {
    /// Syntax error, missing or unknown key. `line` is 1-based when known.
    Toml {
        line: Option<usize>,
        message: String,
    },
    InvalidTime(String),
    DuplicateTime(TimeOfDay),
    /// Title is empty or only whitespace.
    EmptyTitle(TimeOfDay),
    TooFewEntries(usize),
    Io(io::Error),
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml {
                line: Some(line),
                message,
            } => write!(f, "invalid TOML at line {line}: {message}"),
            Self::Toml {
                line: None,
                message,
            } => write!(f, "invalid TOML: {message}"),
            Self::InvalidTime(s) => write!(f, "invalid time \"{s}\" (expected HH:MM, 24-hour)"),
            Self::DuplicateTime(t) => write!(f, "duplicate time {t}"),
            Self::EmptyTitle(t) => write!(f, "empty title at {t}"),
            Self::TooFewEntries(n) => write!(f, "at least 2 entries are required (found {n})"),
            Self::Io(e) => write!(f, "cannot access schedule file: {e}"),
        }
    }
}

impl std::error::Error for ScheduleError {}

impl From<io::Error> for ScheduleError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSchedule {
    #[serde(default)]
    entry: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    time: String,
    title: String,
}

/// Parses and validates schedule TOML.
pub fn parse(src: &str) -> Result<Schedule, ScheduleError> {
    let raw: RawSchedule = toml::from_str(src).map_err(|e| ScheduleError::Toml {
        // Users edit the file by hand, so point them at the line
        line: e
            .span()
            .map(|span| src[..span.start].matches('\n').count() + 1),
        message: e.message().to_owned(),
    })?;
    from_entries(raw.entry.into_iter().map(|e| (e.time, e.title)).collect())
}

/// Validates `(time, title)` pairs with the same rules as the file (Spec §3).
pub fn from_entries(raw: Vec<(String, String)>) -> Result<Schedule, ScheduleError> {
    let mut entries = raw
        .into_iter()
        .map(|(time, title)| {
            let time = TimeOfDay::parse(&time).ok_or(ScheduleError::InvalidTime(time))?;
            if title.trim().is_empty() {
                return Err(ScheduleError::EmptyTitle(time));
            }
            Ok(Entry { time, title })
        })
        .collect::<Result<Vec<_>, ScheduleError>>()?;
    if entries.len() < 2 {
        return Err(ScheduleError::TooFewEntries(entries.len()));
    }
    entries.sort_by_key(|e| e.time);
    if let Some(w) = entries.windows(2).find(|w| w[0].time == w[1].time) {
        return Err(ScheduleError::DuplicateTime(w[0].time));
    }
    Ok(Schedule { entries })
}

/// Serializes the schedule in the same layout as [`DEFAULT_SCHEDULE`].
pub fn to_toml(schedule: &Schedule) -> String {
    schedule
        .entries
        .iter()
        .map(|e| {
            // Let the toml crate quote and escape the title
            let title = toml::Value::String(e.title.clone());
            format!("[[entry]]\ntime = \"{}\"\ntitle = {title}\n", e.time)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Schedule file under the OS data dir (`~/Library/Application Support` on macOS).
/// Not app_data_dir, which uses the bundle identifier instead of "Wane".
pub fn path_in(data_dir: &Path) -> PathBuf {
    data_dir.join("Wane/schedule.toml")
}

/// Loads the schedule, creating the file with the default schedule if it does not exist.
pub fn load_or_create(path: &Path) -> Result<Schedule, ScheduleError> {
    let src = match fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            // create_new so a file saved after the read above is never overwritten
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(mut file) => {
                    file.write_all(DEFAULT_SCHEDULE.as_bytes())?;
                    DEFAULT_SCHEDULE.to_owned()
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => fs::read_to_string(path)?,
                Err(e) => return Err(e.into()),
            }
        }
        Err(e) => return Err(e.into()),
    };
    parse(&src)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> TimeOfDay {
        TimeOfDay::parse(s).unwrap()
    }

    #[test]
    fn parses_entries_and_sorts_by_time() {
        let s = parse(
            r#"
[[entry]]
time = "12:00"
title = "Lunch"

[[entry]]
time = "08:00"
title = "Wake up"
"#,
        )
        .unwrap();
        let got: Vec<_> = s
            .entries()
            .iter()
            .map(|e| (e.time, e.title.as_str()))
            .collect();
        assert_eq!(got, vec![(t("08:00"), "Wake up"), (t("12:00"), "Lunch")]);
    }

    #[test]
    fn time_parses_to_minutes() {
        assert_eq!(t("00:00").minutes(), 0);
        assert_eq!(t("23:59").minutes(), 23 * 60 + 59);
        assert_eq!(t("08:05").to_string(), "08:05");
    }

    #[test]
    fn rejects_bad_time_formats() {
        for bad in [
            "8:00",
            "24:00",
            "12:60",
            "12:5",
            "1200",
            "ab:cd",
            "12:00 ",
            "+1:00",
            "１２:００",
        ] {
            let src = format!(
                "[[entry]]\ntime = \"{bad}\"\ntitle = \"a\"\n[[entry]]\ntime = \"10:00\"\ntitle = \"b\"\n"
            );
            match parse(&src) {
                Err(ScheduleError::InvalidTime(s)) => assert_eq!(s, bad),
                other => panic!("{bad}: expected InvalidTime, got {other:?}"),
            }
        }
    }

    #[test]
    fn rejects_duplicate_times() {
        let src = "[[entry]]\ntime = \"09:00\"\ntitle = \"a\"\n[[entry]]\ntime = \"09:00\"\ntitle = \"b\"\n";
        assert!(matches!(parse(src), Err(ScheduleError::DuplicateTime(x)) if x == t("09:00")));
    }

    #[test]
    fn rejects_fewer_than_two_entries() {
        assert!(matches!(parse(""), Err(ScheduleError::TooFewEntries(0))));
        let one = "[[entry]]\ntime = \"09:00\"\ntitle = \"a\"\n";
        assert!(matches!(parse(one), Err(ScheduleError::TooFewEntries(1))));
    }

    #[test]
    fn rejects_malformed_toml() {
        assert!(matches!(
            parse("[[entry]\n"),
            Err(ScheduleError::Toml { .. })
        ));
        // A missing required key is also a TOML error
        assert!(matches!(
            parse("[[entry]]\ntime = \"09:00\"\n"),
            Err(ScheduleError::Toml { .. })
        ));
    }

    #[test]
    fn toml_error_message_includes_line_number() {
        let src = "[[entry]]\ntime = \"09:00\"\ntitle = \"a\"\n[[entry]]\ntime = \"10:00\"\n";
        let msg = parse(src).unwrap_err().to_string();
        assert!(msg.starts_with("invalid TOML at line 4:"), "{msg}");
        assert!(msg.contains("missing field `title`"), "{msg}");
    }

    #[test]
    fn rejects_unknown_keys() {
        let in_entry = "[[entry]]\ntime = \"09:00\"\ntitle = \"a\"\ntitel = \"x\"\n[[entry]]\ntime = \"10:00\"\ntitle = \"b\"\n";
        assert!(matches!(parse(in_entry), Err(ScheduleError::Toml { .. })));
        let top_level = "foo = 1\n[[entry]]\ntime = \"09:00\"\ntitle = \"a\"\n[[entry]]\ntime = \"10:00\"\ntitle = \"b\"\n";
        assert!(matches!(parse(top_level), Err(ScheduleError::Toml { .. })));
    }

    #[test]
    fn rejects_empty_titles() {
        for bad in ["", "   "] {
            let src = format!(
                "[[entry]]\ntime = \"09:00\"\ntitle = \"{bad}\"\n[[entry]]\ntime = \"10:00\"\ntitle = \"b\"\n"
            );
            assert!(
                matches!(parse(&src), Err(ScheduleError::EmptyTitle(x)) if x == t("09:00")),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn from_entries_applies_the_file_rules() {
        let s = from_entries(vec![
            ("12:00".into(), "Lunch".into()),
            ("08:00".into(), "Wake up".into()),
        ])
        .unwrap();
        assert_eq!(s.entries()[0].time, t("08:00"));
        assert!(matches!(
            from_entries(vec![
                ("8:00".into(), "a".into()),
                ("10:00".into(), "b".into())
            ]),
            Err(ScheduleError::InvalidTime(_))
        ));
        assert!(matches!(
            from_entries(vec![("08:00".into(), "a".into())]),
            Err(ScheduleError::TooFewEntries(1))
        ));
    }

    #[test]
    fn to_toml_round_trips_and_escapes_titles() {
        let s = from_entries(vec![
            ("12:00".into(), "Lunch \"big\"\n# not a comment".into()),
            ("08:00".into(), "Wake up".into()),
        ])
        .unwrap();
        let src = to_toml(&s);
        assert!(src.starts_with("[[entry]]\ntime = \"08:00\"\n"), "{src}");
        assert_eq!(parse(&src).unwrap(), s);
    }

    #[test]
    fn default_schedule_round_trips_byte_for_byte() {
        assert_eq!(to_toml(&parse(DEFAULT_SCHEDULE).unwrap()), DEFAULT_SCHEDULE);
    }

    #[test]
    fn default_schedule_is_valid() {
        let s = parse(DEFAULT_SCHEDULE).unwrap();
        assert_eq!(s.entries().len(), 9);
        assert_eq!(s.entries()[0].time, t("07:00"));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wane-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn load_or_create_writes_default_when_missing() {
        let dir = temp_dir("create");
        let path = dir.join("Wane/schedule.toml");
        let s = load_or_create(&path).unwrap();
        assert_eq!(s, parse(DEFAULT_SCHEDULE).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), DEFAULT_SCHEDULE);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_or_create_keeps_existing_file() {
        let dir = temp_dir("existing");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("schedule.toml");
        let src = "[[entry]]\ntime = \"07:00\"\ntitle = \"a\"\n[[entry]]\ntime = \"10:00\"\ntitle = \"b\"\n";
        fs::write(&path, src).unwrap();
        let s = load_or_create(&path).unwrap();
        assert_eq!(s.entries()[0].time, t("07:00"));
        assert_eq!(fs::read_to_string(&path).unwrap(), src);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_or_create_reports_invalid_existing_file() {
        let dir = temp_dir("invalid");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("schedule.toml");
        fs::write(&path, "").unwrap();
        assert!(matches!(
            load_or_create(&path),
            Err(ScheduleError::TooFewEntries(0))
        ));
        fs::remove_dir_all(dir).unwrap();
    }
}
