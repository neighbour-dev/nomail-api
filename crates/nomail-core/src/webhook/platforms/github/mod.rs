//! GitHub webhook authentication, JSON/form decoding, event model and inbox mapping.
pub mod config;
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformData, PlatformEvent,
    ProtocolResponse, ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto,
    support::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubEvent {
    pub action: Option<String>,
    pub repository: Option<GithubRepository>,
    pub sender: Option<GithubUser>,
    pub issue: Option<GithubIssue>,
    pub comment: Option<GithubComment>,
    pub pull_request: Option<GithubPullRequest>,
    pub workflow_run: Option<GithubWorkflowRun>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubUser {
    pub login: Option<String>,
    pub avatar_url: Option<String>,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubLabel {
    pub name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubIssue {
    pub number: Option<u64>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
    pub html_url: Option<String>,
    pub user: Option<GithubUser>,
    pub labels: Vec<GithubLabel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubComment {
    pub id: Option<u64>,
    pub body: Option<String>,
    pub html_url: Option<String>,
    pub user: Option<GithubUser>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubPullRequest {
    #[serde(flatten)]
    pub issue: GithubIssue,
    pub merged: Option<bool>,
    pub head_branch: Option<String>,
    pub base_branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubWorkflowRun {
    pub id: Option<u64>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub conclusion: Option<String>,
    pub html_url: Option<String>,
    pub head_branch: Option<String>,
    pub head_sha: Option<String>,
    pub run_number: Option<u64>,
}

fn user(value: &Value) -> Option<GithubUser> {
    value.as_object().map(|_| GithubUser {
        login: optional(text(value, &["login"])),
        avatar_url: optional(text(value, &["avatar_url"])),
        html_url: optional(text(value, &["html_url"])),
    })
}

fn issue(value: &Value) -> Option<GithubIssue> {
    value.as_object().map(|_| GithubIssue {
        number: value["number"].as_u64(),
        title: optional(text(value, &["title"])),
        body: value["body"].as_str().map(str::to_owned),
        state: optional(text(value, &["state"])),
        html_url: optional(text(value, &["html_url"])),
        user: user(&value["user"]),
        labels: value["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|label| GithubLabel {
                name: label
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| optional(text(label, &["name"]))),
                color: optional(text(label, &["color"])),
                description: optional(text(label, &["description"])),
            })
            .collect(),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubRepository {
    pub full_name: Option<String>,
    pub html_url: Option<String>,
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::HmacSha256(secret) if !secret.is_empty() => {
            crypto::hmac_header(secret, "x-hub-signature-256", request)?
        }
        _ => return Err(invalid_auth()),
    }
    let payload = if is_form(request) {
        let fields = form(request.body)?;
        let encoded = fields
            .get("payload")
            .and_then(Value::as_str)
            .ok_or(WebhookError::InvalidPayload("missing GitHub form payload"))?;
        object(encoded.as_bytes())?
    } else {
        object(request.body)?
    };
    let event_type = required(
        request.header("x-github-event").into(),
        "missing GitHub event type",
    )?;
    if event_type == "ping" {
        return Ok(ReceiveOutcome::Respond(ProtocolResponse {
            status: 200,
            content_type: "text/plain".into(),
            body: b"pong".to_vec(),
        }));
    }
    let event = GithubEvent {
        action: optional(text(&payload, &["action"])),
        repository: payload
            .get("repository")
            .filter(|value| value.is_object())
            .map(|value| GithubRepository {
                full_name: optional(text(value, &["full_name"])),
                html_url: optional(text(value, &["html_url"])),
            }),
        sender: user(&payload["sender"]),
        issue: issue(&payload["issue"]),
        comment: payload["comment"].as_object().map(|_| GithubComment {
            id: payload["comment"]["id"].as_u64(),
            body: payload["comment"]["body"].as_str().map(str::to_owned),
            html_url: optional(text(&payload, &["comment", "html_url"])),
            user: user(&payload["comment"]["user"]),
            created_at: optional(text(&payload, &["comment", "created_at"])),
        }),
        pull_request: issue(&payload["pull_request"]).map(|issue| GithubPullRequest {
            issue,
            merged: payload["pull_request"]["merged"].as_bool(),
            head_branch: optional(text(&payload, &["pull_request", "head", "ref"])),
            base_branch: optional(text(&payload, &["pull_request", "base", "ref"])),
        }),
        workflow_run: payload["workflow_run"]
            .as_object()
            .map(|_| GithubWorkflowRun {
                id: payload["workflow_run"]["id"].as_u64(),
                name: optional(text(&payload, &["workflow_run", "name"])),
                status: optional(text(&payload, &["workflow_run", "status"])),
                conclusion: optional(text(&payload, &["workflow_run", "conclusion"])),
                html_url: optional(text(&payload, &["workflow_run", "html_url"])),
                head_branch: optional(text(&payload, &["workflow_run", "head_branch"])),
                head_sha: optional(text(&payload, &["workflow_run", "head_sha"])),
                run_number: payload["workflow_run"]["run_number"].as_u64(),
            }),
    };
    Ok(PlatformEvent {
        metadata: metadata(
            event_type,
            None,
            "github:delivery",
            request.header("x-github-delivery").into(),
        ),
        data: PlatformData::Github(Box::new(event)),
        raw: EventContent::Json(payload),
    }
    .into())
}

impl GithubEvent {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let p = raw.as_json().unwrap_or(&Value::Null);
        let kind = metadata.event_type.as_str();
        let repository = self
            .repository
            .as_ref()
            .and_then(|repo| repo.full_name.as_deref())
            .unwrap_or("");
        let mut title = format!("GitHub {kind}");
        for part in [
            self.action.clone().unwrap_or_default(),
            repository.to_owned(),
        ] {
            if !part.is_empty() {
                title.push_str(&format!(": {part}"));
            }
        }
        let (content, url) = match kind {
            "issues" => (
                text(p, &["issue", "title"]),
                text(p, &["issue", "html_url"]),
            ),
            "issue_comment" | "pull_request_review_comment" => (
                text(p, &["comment", "body"]),
                text(p, &["comment", "html_url"]),
            ),
            "pull_request" | "pull_request_review" => (
                text(p, &["pull_request", "title"]),
                text(p, &["pull_request", "html_url"]),
            ),
            "push" => (text(p, &["head_commit", "message"]), text(p, &["compare"])),
            "workflow_run" => (
                text(p, &["workflow_run", "name"]),
                text(p, &["workflow_run", "html_url"]),
            ),
            _ => (String::new(), String::new()),
        };
        message(
            title,
            first(&[content, raw.to_string()]),
            first(&[
                repository.to_owned(),
                self.sender
                    .as_ref()
                    .and_then(|user| user.login.clone())
                    .unwrap_or_default(),
                "github".into(),
            ]),
            // Outcomes stay in workflow_run.conclusion, not NoMail priority.
            "normal",
            first(&[
                url,
                self.repository
                    .as_ref()
                    .and_then(|repo| repo.html_url.clone())
                    .unwrap_or_default(),
            ]),
        )
    }
}
