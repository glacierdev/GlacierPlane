use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GitHubWebhookPayload {
    pub r#ref: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    pub repository: RepositoryInfo,
    #[serde(default)]
    pub commits: Vec<CommitInfo>,
    #[serde(default)]
    pub head_commit: Option<CommitInfo>,
    #[serde(default)]
    pub pull_request: Option<PullRequestInfo>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub number: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RepositoryInfo {
    #[allow(dead_code)]
    pub full_name: String,
    pub clone_url: String,
    #[serde(default)]
    pub ssh_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: CommitAuthor,
}

#[derive(Debug, Deserialize)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PullRequestInfo {
    pub number: Option<i64>,
    pub title: Option<String>,
    #[serde(default)]
    pub draft: Option<bool>,
    pub head: PullRequestHead,
    pub base: PullRequestBase,
    pub user: Option<PullRequestUser>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PullRequestHead {
    pub r#ref: String,
    pub sha: String,
    #[serde(default)]
    pub repo: Option<PullRequestRepo>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PullRequestBase {
    pub r#ref: String,
    pub sha: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PullRequestRepo {
    pub clone_url: Option<String>,
    pub ssh_url: Option<String>,
    pub full_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PullRequestUser {
    pub login: String,
}
