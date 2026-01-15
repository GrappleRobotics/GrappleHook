use chrono::{DateTime, Utc};
use reqwest::header::USER_AGENT;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct LightReleaseResponse {
    pub name: String,
    pub tag_name: String,
    pub published_at: String,
    pub html_url: String,
}

pub async fn most_recent_update_available<F: Fn(&LightReleaseResponse) -> bool>(
    repo: &str,
    acceptance_filter: F,
) -> anyhow::Result<Option<LightReleaseResponse>> {
    let client = reqwest::Client::new();

    let response: Vec<LightReleaseResponse> = client
        .get(repo)
        .header(USER_AGENT, "GrappleHook")
        .send()
        .await?
        .json()
        .await?;

    let mut most_recent: Option<(LightReleaseResponse, DateTime<Utc>)> = None;

    for r in response {
        if acceptance_filter(&r) {
            let dt = DateTime::parse_from_rfc3339(&r.published_at).ok();

            if let Some(dt) = dt {
                if most_recent.is_none() || most_recent.as_ref().unwrap().1 < dt {
                    most_recent = Some((r, dt.into()))
                }
            }
        }
    }

    Ok(most_recent.map(|x| x.0))
}
