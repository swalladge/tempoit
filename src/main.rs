use std::io::{stdin, stdout, Write};

use chrono::Duration;
use confy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use structopt::StructOpt;
use regex::Regex;

use tempoit::jira::{duration_to_jira, JiraClient};
use tempoit::timew::TimewClient;

fn deserialize_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let regex_str = String::deserialize(deserializer)?;
    Regex::new(&regex_str).map_err(serde::de::Error::custom)
}

fn serialize_regex<S>(re: &Regex, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(re.as_str())
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    username: String,
    password: String,
    base_url: String,
    #[serde(deserialize_with = "deserialize_regex")]
    #[serde(serialize_with = "serialize_regex")]
    ticket_regex: Regex,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: "user".to_owned(),
            password: "pass".to_owned(),
            base_url: "https://tasks.opencraft.com".to_owned(),
            ticket_regex: Regex::new(r"^(?i:FAL|SE|BB|OC|MNG|BIZ|ADMIN)-\d+$").expect("default regex is invalid"),
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "tempoit", about = "Upload worklogs to jira from timew export")]
struct Opt {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _opt = Opt::from_args();
    let cfg: Config = confy::load("tempoit")?;
    let logs_client = TimewClient::new(cfg.ticket_regex);

    let parsed_intervals = logs_client.get_worklogs()?;

    // Check for errors. Display and exit if found any.
    let mut worklogs = vec![];
    for maybe_worklog in parsed_intervals {
        match maybe_worklog {
            Err(s) => {
                println!("{}", s);
            }
            Ok(w) => {
                worklogs.push(w);
            }
        }
    }

    if worklogs.len() == 0 {
        println!(":: No worklogs to upload.");
        return Ok(());
    }

    println!(":: Ready to upload worklogs:");
    for worklog in worklogs.iter() {
        println!("   {}", worklog);
    }
    println!(
        ":: Total time: {}",
        duration_to_jira(
            &worklogs
                .iter()
                .map(|x| x.duration)
                .fold(Duration::seconds(0), |acc, x| acc + x.clone())
        )
    );
    print!(":: Confirm upload [y/N] ");
    stdout().flush()?;

    let mut response = String::new();
    stdin().read_line(&mut response)?;
    let response = response.trim().to_lowercase();
    if response != "y" {
        println!(":: Canceled by user, Aborting.");
        return Ok(());
    }

    let jira_client = JiraClient::new(&cfg.base_url, &cfg.username, &cfg.password).await?;

    let mut failed_uploads = vec![];
    for worklog in worklogs.iter() {
        print!(":: Uploading {}... ", worklog);
        match jira_client.add_worklog(worklog).await {
            Err(e) => {
                println!("FAIL");
                println!("{}", e);
                failed_uploads.push(worklog);
                logs_client.record_fail(&worklog.id)?;
            }
            Ok(_) => {
                println!("SUCCESS");
                logs_client.record_success(&worklog.id)?;
            }
        }
    }

    if failed_uploads.len() > 0 {
        println!(":: Some worklogs failed to upload. Please try again:");
        for worklog in failed_uploads {
            println!("   {}", worklog);
        }
        return Err("Upload complete with errors.".into());
    }

    Ok(())
}
