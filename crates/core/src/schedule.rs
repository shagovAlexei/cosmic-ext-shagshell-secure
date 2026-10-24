//! Script scheduler: jobs in `schedule.toml`, run while the app is open. What is due is a pure
//! function of the job, when it last ran and the local time (minutes since the epoch).

use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Every {
    /// Every N minutes (1 or more).
    Minutes { n: u32 },
    /// Every day at `hour:minute`, local time.
    Daily { hour: u8, minute: u8 },
}

impl Default for Every {
    fn default() -> Self {
        Every::Minutes { n: 60 }
    }
}

impl Every {
    /// Out-of-range numbers pulled in (0 minutes → 1, 25:61 → 23:59).
    pub fn fixed(self) -> Self {
        match self {
            Every::Minutes { n } => Every::Minutes { n: n.max(1) },
            Every::Daily { hour, minute } => Every::Daily {
                hour: hour.min(23),
                minute: minute.min(59),
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Job {
    pub name: String,
    pub script: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    pub every: Every,
    pub enabled: bool,
}

/// Is `job` due at `now`? `last` = when it last ran (or when counting began: the app's start,
/// the job's save), so what passed while the app was closed is not made up for.
pub fn due(job: &Job, last: i64, now: i64) -> bool {
    if !job.enabled || now <= last {
        return false;
    }
    match job.every.fixed() {
        Every::Minutes { n } => now - last >= i64::from(n),
        Every::Daily { hour, minute } => {
            let at = now.div_euclid(1440) * 1440 + i64::from(hour) * 60 + i64::from(minute);
            last < at && at <= now
        }
    }
}

/// Local time as minutes since the epoch (a day is always 1440 of them: zone shifts are ignored).
pub fn local_minutes() -> i64 {
    let z = jiff::Zoned::now();
    (z.timestamp().as_second() + i64::from(z.offset().seconds())).div_euclid(60)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    #[serde(rename = "job")]
    pub jobs: Vec<Job>,
}

impl Schedule {
    pub fn default_path() -> std::path::PathBuf {
        crate::buttonbar::default_path().with_file_name("schedule.toml")
    }

    /// Missing: empty. A broken file is an error, never an empty list to be written back.
    pub fn load(path: &Path) -> io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string(self).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(every: Every) -> Job {
        Job {
            name: "j".into(),
            script: "/s.py".into(),
            args: vec![],
            every,
            enabled: true,
        }
    }

    #[test]
    fn interval_fires_after_n_minutes() {
        let j = job(Every::Minutes { n: 5 });
        assert!(!due(&j, 100, 104));
        assert!(due(&j, 100, 105));
        assert!(!due(&j, 100, 100));
    }

    #[test]
    fn daily_fires_once_at_its_time() {
        let j = job(Every::Daily {
            hour: 9,
            minute: 30,
        });
        let day = 20_000 * 1440;
        let at = day + 9 * 60 + 30;
        assert!(!due(&j, day, at - 1));
        assert!(due(&j, day, at));
        assert!(due(&j, day, at + 20));
        // Ran at the time: not again that day, again the next.
        assert!(!due(&j, at, at + 5));
        assert!(due(&j, at, at + 1440));
    }

    #[test]
    fn daily_is_not_made_up_for_when_counting_began_later() {
        let j = job(Every::Daily { hour: 9, minute: 0 });
        let day = 20_000 * 1440;
        // The app started at 15:00: today's 09:00 is gone.
        assert!(!due(&j, day + 15 * 60, day + 15 * 60 + 1));
    }

    #[test]
    fn disabled_and_out_of_range() {
        let mut j = job(Every::Minutes { n: 0 });
        assert!(due(&j, 0, 1), "0 minutes counts as 1");
        j.enabled = false;
        assert!(!due(&j, 0, 100));
        assert_eq!(
            Every::Daily {
                hour: 30,
                minute: 99
            }
            .fixed(),
            Every::Daily {
                hour: 23,
                minute: 59
            }
        );
    }

    #[test]
    fn file_round_trip_and_broken_file_is_an_error() {
        let dir = std::env::temp_dir().join(format!("shagshell-sched-{}", std::process::id()));
        let p = dir.join("schedule.toml");
        assert_eq!(Schedule::load(&p).unwrap(), Schedule::default());
        let s = Schedule {
            jobs: vec![
                job(Every::Minutes { n: 15 }),
                job(Every::Daily { hour: 7, minute: 5 }),
            ],
        };
        s.save(&p).unwrap();
        assert_eq!(Schedule::load(&p).unwrap(), s);
        std::fs::write(&p, "job = 5 [").unwrap();
        assert!(Schedule::load(&p).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
