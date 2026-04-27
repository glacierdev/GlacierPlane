use crate::db::{Build, Database};
use serde::Serialize;

#[derive(Clone)]
pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
}

#[derive(Serialize)]
struct CreateStatusRequest<'a> {
    state: &'a str,
    description: &'a str,
    context: &'a str,
}

impl GitHubClient {
    pub fn new(token: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("control-plane")
            .build()
            .expect("failed to build HTTP client");
        Self { client, token }
    }

    async fn post_commit_status(
        &self,
        owner_repo: &str,
        sha: &str,
        state: &str,
        description: &str,
        context: &str,
    ) {
        let url = format!(
            "https://api.github.com/repos/{}/statuses/{}",
            owner_repo, sha
        );

        let body = CreateStatusRequest {
            state,
            description,
            context,
        };

        let short_sha = &sha[..sha.len().min(7)];

        match self
            .client
            .post(&url)
            .header("Authorization", format!("token {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    "GitHub status '{}' posted for {}/{}",
                    state,
                    owner_repo,
                    short_sha
                );
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    "GitHub status API returned {} for {}/{}: {}",
                    status,
                    owner_repo,
                    short_sha,
                    text
                );
            }
            Err(e) => {
                tracing::error!("Failed to post GitHub commit status: {}", e);
            }
        }
    }
}

fn build_status_to_github(status: &str) -> Option<&'static str> {
    match status {
        "scheduled" | "running" => Some("pending"),
        "passed" => Some("success"),
        "failed" => Some("failure"),
        _ => None,
    }
}

fn build_description(status: &str, number: i32) -> String {
    match status {
        "scheduled" => format!("Build #{} is queued", number),
        "running" => format!("Build #{} is running", number),
        "passed" => format!("Build #{} passed", number),
        "failed" => format!("Build #{} failed", number),
        _ => format!("Build #{}", number),
    }
}

fn extract_github_owner_repo(url: &str) -> Option<String> {
    if !url.contains("github.com") {
        return None;
    }

    let mut s = url.to_string();
    if s.ends_with(".git") {
        s.truncate(s.len() - 4);
    }
    if s.ends_with('/') {
        s.truncate(s.len() - 1);
    }

    if s.starts_with("git@") {
        let colon_pos = s.find(':')?;
        let path = &s[colon_pos + 1..];
        if path.contains('/') {
            return Some(path.to_string());
        }
    } else {
        let without_scheme = s
            .strip_prefix("https://")
            .or_else(|| s.strip_prefix("http://"))
            .unwrap_or(&s);
        let after_host = without_scheme.strip_prefix("github.com/")?;
        if after_host.contains('/') {
            return Some(after_host.to_string());
        }
    }

    None
}

pub async fn notify_build_status(github: &GitHubClient, db: &Database, build: &Build) {
    let gh_state = match build_status_to_github(&build.status) {
        Some(s) => s,
        None => return,
    };

    let pipeline = match db.get_pipeline_by_slug(&build.pipeline_slug).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "Cannot notify GitHub: pipeline '{}' lookup failed: {}",
                build.pipeline_slug,
                e
            );
            return;
        }
    };

    let owner_repo = match extract_github_owner_repo(&pipeline.repository_url) {
        Some(or) => or,
        None => return,
    };

    let description = build_description(&build.status, build.number);
    let context = format!("ci/{}", build.pipeline_slug);

    github
        .post_commit_status(&owner_repo, &build.commit, gh_state, &description, &context)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_https_url() {
        assert_eq!(
            extract_github_owner_repo("https://github.com/acme/repo"),
            Some("acme/repo".into())
        );
    }

    #[test]
    fn extract_https_url_with_git_suffix() {
        assert_eq!(
            extract_github_owner_repo("https://github.com/acme/repo.git"),
            Some("acme/repo".into())
        );
    }

    #[test]
    fn extract_ssh_url() {
        assert_eq!(
            extract_github_owner_repo("git@github.com:acme/repo.git"),
            Some("acme/repo".into())
        );
    }

    #[test]
    fn extract_ssh_url_no_git_suffix() {
        assert_eq!(
            extract_github_owner_repo("git@github.com:acme/repo"),
            Some("acme/repo".into())
        );
    }

    #[test]
    fn non_github_url_returns_none() {
        assert_eq!(
            extract_github_owner_repo("https://gitlab.com/acme/repo"),
            None
        );
    }

    #[test]
    fn trailing_slash_stripped() {
        assert_eq!(
            extract_github_owner_repo("https://github.com/acme/repo/"),
            Some("acme/repo".into())
        );
    }

    #[test]
    fn maps_scheduled_to_pending() {
        assert_eq!(build_status_to_github("scheduled"), Some("pending"));
    }

    #[test]
    fn maps_running_to_pending() {
        assert_eq!(build_status_to_github("running"), Some("pending"));
    }

    #[test]
    fn maps_passed_to_success() {
        assert_eq!(build_status_to_github("passed"), Some("success"));
    }

    #[test]
    fn maps_failed_to_failure() {
        assert_eq!(build_status_to_github("failed"), Some("failure"));
    }

    #[test]
    fn unknown_status_returns_none() {
        assert_eq!(build_status_to_github("canceled"), None);
    }
}
