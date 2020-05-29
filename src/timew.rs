use std::error::Error;
use std::fmt;
use std::process::Command;
use std::str;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Deserializer};

use crate::jira::Worklog;

fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let id = u32::deserialize(deserializer)?;
    Ok(format!("@{}", id))
}

fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let date_string = String::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(&date_string, "%Y%m%dT%H%M%SZ")
        .map_err(serde::de::Error::custom)
        .map(|x| DateTime::<Utc>::from_utc(x, Utc))
}

fn deserialize_option_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let option_date_string = Option::<String>::deserialize(deserializer)?;
    option_date_string
        .map(|x| {
            NaiveDateTime::parse_from_str(&x, "%Y%m%dT%H%M%SZ")
                .map_err(serde::de::Error::custom)
                .map(|x| DateTime::<Utc>::from_utc(x, Utc))
        })
        .transpose()
}

// All datetimes output from timew export are in utc.
#[derive(Debug, Deserialize)]
struct Interval {
    #[serde(deserialize_with = "deserialize_id")]
    id: String,
    #[serde(deserialize_with = "deserialize_datetime")]
    start: DateTime<Utc>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_option_datetime")]
    end: Option<DateTime<Utc>>,
    #[serde(default)]
    tags: Vec<String>,
    annotation: Option<String>,
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let date = self.end.unwrap_or(self.start).date().format("%Y-%m-%d");
        let duration = match self.end {
            Some(end) => {
                let duration = end - self.start;
                format!(
                    "{}h {}m",
                    duration.num_hours(),
                    duration.num_minutes() - duration.num_hours() * 60
                )
            }
            None => "open".to_owned(),
        };

        write!(
            f,
            "{id:<5} {date} {duration:7} [{tags:^15}] '{annotation}'",
            date = date,
            id = self.id,
            tags = self.tags.join(", "),
            annotation = self.annotation.clone().unwrap_or("-".to_owned()),
            duration = duration,
        )
    }
}

fn parse_interval(interval: &Interval) -> Result<Worklog, String> {
    let end = match interval.end {
        Some(end) => end,
        None => {
            return Err(format!("INFO( open ): {}", interval));
        }
    };

    let duration = end - interval.start;
    let date = end.with_timezone(&Local).date().naive_local();

    // TODO:  make this regex configurable from config file
    let re = Regex::new(r"^(?i:SE|BB|OC|MNG|BIZ|ADMIN)-\d+$").expect("regex invalid");
    let issue = match interval.tags.iter().find(|x| re.is_match(x)) {
        Some(issue) => issue.to_uppercase(),
        None => {
            return Err(format!("ERR( untagged ): {}", interval));
        }
    };

    let id = interval.id.to_string();

    let description = match interval.annotation.clone() {
        None => {
            return Err(format!("ERR(no ann): {}", interval));
        }
        Some(ann) => ann,
    };

    return Ok(Worklog {
        id,
        duration,
        date,
        issue,
        description,
    });
}

type ClientResult<T> = Result<T, Box<dyn Error>>;

pub struct TimewClient {}

impl TimewClient {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_worklogs(&self) -> ClientResult<Vec<Result<Worklog, String>>> {
        // TODO: make this command configurable
        let proc = Command::new("timew")
            .args(&["export", "oc", "log"])
            .output()?;
        let export_contents = str::from_utf8(&proc.stdout)?;
        let intervals: Vec<Interval> = serde_json::from_str(export_contents)?;

        Ok(intervals.iter().map(parse_interval).collect())
    }

    pub fn record_success(&self, id: &str) -> ClientResult<()> {
        run("timew", &["tag", id, "logged"])?;
        run("timew", &["untag", id, "log", "logfail"])
    }

    pub fn record_fail(&self, id: &str) -> ClientResult<()> {
        run("timew", &["tag", id, "logfail"])
    }
}

/// Helper function to spawn and run a command, returning an error if did not exit cleanly.
pub fn run(cmd: &str, args: &[&str]) -> Result<(), Box<dyn Error>> {
    println!("RUN {} {:?}", cmd, args);
    let status = Command::new(cmd).args(args).status()?;
    match status.success() {
        true => Ok(()),
        false => Err(format!("Command exited with {}", status).into()),
    }
}
