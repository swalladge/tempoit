use std::io::{stdin, stdout, Write};

use chrono::Duration;
use confy;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use tempoit::jira::{duration_to_jira, JiraClient};
use tempoit::timew::TimewClient;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    username: String,
    password: String,
    base_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: "user".to_owned(),
            password: "pass".to_owned(),
            base_url: "https://tasks.opencraft.com".to_owned(),
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
    let logs_client = TimewClient::new();

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
