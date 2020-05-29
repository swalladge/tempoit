use std::fmt;

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize, Serializer};

const LOGIN_ENDPOINT: &str = "/rest/gadget/1.0/login";
const WORKLOGS_ENDPOINT: &str = "/rest/tempo-rest/1.0/worklogs/";

pub(crate) fn serialize_date<S>(date: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&date.format("%Y-%m-%d").to_string())
}

fn serialize_worklog_duration<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&duration_to_jira(duration))
}

/// Represents a worklog to be uploaded to Tempo.
#[derive(Debug, Clone)]
pub struct Worklog {
    /// The time spent working. Jira takes these values in a simple `Xh Xm` format. The api methods
    /// will convert a high fidelity duration to `Xm` for you, rounding to the nearest minute.
    pub duration: Duration,

    /// The date on which to log the time. This is a naive date because all it needs is a date in
    /// YYYY-MM-DD format. If you are not on UTC, then you will need to account for off-by-one
    /// errors yourself.
    pub date: NaiveDate,

    /// A Jira ticket id. For example, `"SE-1234"`.
    pub issue: String,

    /// Worklog description - usually a very basic summary of the type of work done. For example,
    /// `"comms"`, or `"implement feature and open PR"`. Must not be an empty string.
    /// TODO: ensure struct cannot be initiated with empty string
    pub description: String,

    /// Represents an id linked to the tool used the log the time. For example, for adding a tag to
    /// timewarrior with `tt tag <id> <tag>` on successful upload.
    /// TODO: this probably should be named better; it's not related to any tempo/jira IDs.
    pub id: String,
}

impl fmt::Display for Worklog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{id:<5} {date} {duration:7} [{issue}] '{desc}'",
            date = self.date,
            duration = duration_to_jira(&self.duration),
            issue = self.issue,
            desc = self.description,
            id = self.id,
        )
    }
}

#[derive(Serialize, Debug)]
struct LoginForm<'a> {
    os_username: &'a str,
    os_password: &'a str,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    allow_cookies: bool,
    captcha_failure: bool,
    communication_error: bool,
    contact_admin_link: String,
    external_user_management: bool,
    is_elevated_security_check_shown: bool,
    is_public_mode: bool,
    login_error: bool,
    login_failed_by_permissions: bool,
    login_succeeded: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WorklogActionType {
    LogTime,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WorklogUpdateForm {
    action_type: WorklogActionType,
    #[serde(serialize_with = "serialize_date")]
    ansidate: NaiveDate,
    selected_user: String,
    #[serde(serialize_with = "serialize_worklog_duration")]
    time: Duration,
    remaining_estimate: String,
    comment: String,
}

/// Client for interacting with the Jira/Tempo api.
pub struct JiraClient {
    client: reqwest::Client,
    username: String,
    base_url: String,
}

impl JiraClient {
    /// Build a new client given the `base_url` of the Jira instance. The `base_url` must be
    /// without a trailing slash. For example: `"https://tasks.opencraft.com"`.
    /// This function will attempt to login. If successful, it will return a `JiraClient` with a
    /// logged in session ready to make api calls. If not, it will return an `Err`.
    ///
    /// ```rust
    /// # use tempoit::jira::JiraClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = JiraClient::new("https://example.com", "my_user", "hunter2").await;
    /// # assert_eq!(client.is_err(), true);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        base_url: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let login_form = LoginForm {
            os_username: username,
            os_password: password,
        };
        let client = reqwest::Client::builder().cookie_store(true).build()?;
        let res = client
            .post(&format!("{}{}", base_url, LOGIN_ENDPOINT))
            .form(&login_form)
            .send()
            .await?
            .error_for_status()?;
        let data: LoginResponse = res.json().await?;
        if data.login_succeeded {
            Ok(Self {
                client,
                username: username.to_owned(),
                base_url: base_url.to_owned(),
            })
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "login failed").into())
        }
    }

    /// Helper function to get the remaining estimate for a ticket after a `worklog` is uploaded.
    /// For example, if a ticket has `1h` estimated left, and you want to upload a worklog of `40m`,
    /// then this function will return `20m`. We need to do this, because the remaining estimate
    /// must be added to the call to add a worklog, otherwise the remaining estimate is not
    /// updated. We also can't calculate it ourselves, because we don't know an api endpoint to get
    /// the ticket info. (TODO: this would be a good addition)
    async fn get_remaining_estimate(
        &self,
        worklog: &Worklog,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // This calculates the time remaining to set if 10m work to be added to SE-2424
        // https://tasks.opencraft.com/rest/tempo-rest/1.0/worklogs/remainingEstimate/calculate/SE-2552/2019-05-29/2019-05-29/3m
        let estimate_url = format!(
            "{base_url}/rest/tempo-rest/1.0/worklogs/remainingEstimate/calculate/{issue}/{date}/{date}/{duration}?username={username}",
            base_url=self.base_url,
            issue=worklog.issue,
            date=worklog.date.format("%Y-%m-%d").to_string(),
            duration=duration_to_jira(&worklog.duration),
            username=&self.username,
        );
        let estimate_response = self
            .client
            .get(&estimate_url)
            .send()
            .await?
            .error_for_status()?;
        Ok(estimate_response.text().await?)
    }

    /// Upload a worklog to Tempo. Note that this is _not_ idempotent; If called twice, this will
    /// add two identical worklogs to tempo. There is also no known way to retrieve the id of the
    /// worklog once uploaded, so it is impossible to find to modify or delete programmatically.
    pub async fn add_worklog(&self, worklog: &Worklog) -> Result<(), Box<dyn std::error::Error>> {
        let form = WorklogUpdateForm {
            action_type: WorklogActionType::LogTime,
            ansidate: worklog.date,
            selected_user: self.username.clone(),
            time: worklog.duration,
            remaining_estimate: self.get_remaining_estimate(&worklog).await?,
            comment: worklog.description.clone(),
        };

        let worklog_response = self
            .client
            .post(&format!(
                "{}{}{}",
                self.base_url, WORKLOGS_ENDPOINT, worklog.issue
            ))
            .form(&form)
            .send()
            .await?
            .error_for_status()?;
        let response_text = worklog_response.text().await?;

        if response_text.find("valid=\"true\"").is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Err: {}", response_text),
            )
            .into());
        }

        Ok(())
    }
}

/// Convert a duration to a jira string. Rounds to the nearest minute. If duration rounded to
/// nearest minute would be zero, then return 1m.
pub fn duration_to_jira(duration: &Duration) -> String {
    let hours = duration.num_hours();
    let minutes = ((duration.num_seconds() as f32 / 60.0).round() as i64) - (hours * 60);
    let safe_minutes = if hours == 0 && minutes == 0 {
        1
    } else {
        minutes
    };
    format!("{}h {}m", hours, safe_minutes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_to_jira_test() {
        // Need to test this to check that rounding is correct.
        assert_eq!(duration_to_jira(&Duration::hours(1)), "1h 0m".to_owned());
        assert_eq!(
            duration_to_jira(&Duration::minutes(23)),
            "0h 23m".to_owned()
        );
        assert_eq!(
            duration_to_jira(&Duration::minutes(123)),
            "2h 3m".to_owned()
        );
        assert_eq!(
            duration_to_jira(&Duration::seconds(125)),
            "0h 2m".to_owned()
        );
        assert_eq!(
            duration_to_jira(&Duration::seconds(149)),
            "0h 2m".to_owned()
        );
        assert_eq!(
            duration_to_jira(&Duration::seconds(150)),
            "0h 3m".to_owned()
        );
        assert_eq!(
            duration_to_jira(&Duration::seconds(151)),
            "0h 3m".to_owned()
        );
        assert_eq!(
            duration_to_jira(&Duration::seconds(180)),
            "0h 3m".to_owned()
        );
        assert_eq!(
            duration_to_jira(&Duration::seconds(181)),
            "0h 3m".to_owned()
        );
        assert_eq!(duration_to_jira(&Duration::seconds(0)), "0h 1m".to_owned());
        assert_eq!(duration_to_jira(&Duration::seconds(5)), "0h 1m".to_owned());
    }
}
