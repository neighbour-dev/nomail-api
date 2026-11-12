//! Native GitHub payload fields. Extraction is field-local and deliberately tolerant;
//! these are not strict upstream DTOs. Unknown/malformed fields remain in raw.
use serde::{Deserialize, Serialize};
use serde_json::Value;

trait GithubValue: Sized {
    fn read(value: &Value) -> Option<Self>;
}

impl GithubValue for String {
    fn read(value: &Value) -> Option<Self> {
        value.as_str().map(str::to_owned)
    }
}

impl GithubValue for u64 {
    fn read(value: &Value) -> Option<Self> {
        value.as_u64()
    }
}

impl GithubValue for bool {
    fn read(value: &Value) -> Option<Self> {
        value.as_bool()
    }
}

impl GithubValue for Value {
    fn read(value: &Value) -> Option<Self> {
        (!value.is_null()).then(|| value.clone())
    }
}

trait Field {
    type Output;
    fn extract(value: &Value) -> Option<Self::Output>;
}

impl<T: GithubValue> Field for Option<T> {
    type Output = T;
    fn extract(value: &Value) -> Option<T> {
        T::read(value)
    }
}

impl<T: GithubValue> Field for Vec<T> {
    type Output = Vec<T>;
    fn extract(value: &Value) -> Option<Vec<T>> {
        Some(value.as_array()?.iter().filter_map(T::read).collect())
    }
}

fn body(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

macro_rules! field {
    ($value:ident, $key:expr, $ty:ty) => {
        crate::webhook::platforms::native::read_json_field($value, $key, <$ty as Field>::extract)
    };
    ($value:ident, $key:expr, $ty:ty, $extract:expr) => {
        crate::webhook::platforms::native::read_json_field($value, $key, |_| ($extract)($value))
    };
}

macro_rules! model {
    ($name:ident $( [$flat:ident: $flat_ty:ty] )? { $($(#[serde(rename = $key:literal)])? $field:ident: $container:ident<$($ty:ty),+> $(= $extract:expr)?),* $(,)? }) => {
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub struct $name {
            $(#[serde(flatten)] pub $flat: $flat_ty,)?
            $($(#[serde(rename = $key)])?
            #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::webhook::platforms::native::deserialize_present")]
            pub $field: payload_field_type!($container<$($ty),+>),)*
        }

        impl GithubValue for $name {
            fn read(value: &Value) -> Option<Self> {
                value.as_object()?;
                Some(Self {
                    $($flat: <$flat_ty>::read(value).unwrap_or_default(),)?
                    $($field: field!(value, payload_field_key!($field $(, $key)?), $container<$($ty),+> $(, $extract)?),)*
                })
            }
        }
    };
}

/// GitHub repository timestamps are Unix seconds in push events and strings in
/// other events. Preserve their wire representation rather than guessing a clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum GithubTimestamp {
    Unix(i64),
    Text(String),
}

impl GithubValue for GithubTimestamp {
    fn read(value: &Value) -> Option<Self> {
        value
            .as_i64()
            .map(Self::Unix)
            .or_else(|| value.as_str().map(|s| Self::Text(s.to_owned())))
    }
}

model!(GithubWebhookPayload {
    action: Option<String>,
    repository: Option<GithubRepository>,
    sender: Option<GithubUser>,
    issue: Option<GithubIssue>,
    comment: Option<GithubComment>,
    pull_request: Option<GithubPullRequest>,
    workflow_run: Option<GithubWorkflowRun>,
    review: Option<GithubReview>,
    thread: Option<GithubReviewThread>,
    workflow: Option<GithubWorkflow>,
    workflow_job: Option<GithubWorkflowJob>,
    check_run: Option<GithubCheckRun>,
    check_suite: Option<GithubCheckSuite>,
    release: Option<GithubRelease>,
    discussion: Option<GithubDiscussion>,
    deployment: Option<GithubDeployment>,
    deployment_status: Option<GithubDeploymentStatus>,
    label: Option<GithubLabel>,
    assignee: Option<GithubUser>,
    milestone: Option<GithubMilestone>,
    changes: Option<GithubChanges>,
    answer: Option<GithubComment>,
    old_answer: Option<GithubComment>,
    number: Option<u64>,
    before: Option<String>,
    after: Option<String>,
    updated_at: Option<String>,
    starred_at: Option<String>,
    #[serde(rename = "ref")]
    ref_name: Option<String> = |v: &Value| String::read(&v["ref"]),
    base_ref: Option<String>, created: Option<bool>, deleted: Option<bool>, forced: Option<bool>, compare: Option<String>,
    pusher: Option<GithubCommitAuthor>, commits: Vec<GithubCommit>, head_commit: Option<GithubCommit>,
    ref_type: Option<String>, master_branch: Option<String>, pusher_type: Option<String>,
    description: Option<String>, id: Option<u64>, state: Option<String>, context: Option<String>,
    sha: Option<String>, target_url: Option<String>, name: Option<String>, avatar_url: Option<String>,
    created_at: Option<String>, commit: Option<GithubStatusCommit>, branches: Vec<GithubStatusBranch>,
});

impl crate::webhook::platforms::native::NativeValue for GithubWebhookPayload {
    fn read(value: &Value) -> Option<Self> {
        <Self as GithubValue>::read(value)
    }
}

model!(GithubUser {
    id: Option<u64>, node_id: Option<String>, login: Option<String>,
    avatar_url: Option<String>, html_url: Option<String>,
    #[serde(rename = "type")]
    user_type: Option<String> = |v: &Value| String::read(&v["type"]),
    site_admin: Option<bool>, user_view_type: Option<String>,
});

model!(GithubLabel {
    id: Option<u64>, node_id: Option<String>, name: Option<String>,
    color: Option<String>, description: Option<String>, url: Option<String>,
    default: Option<bool>, archived_at: Option<String>, archived_by: Option<GithubUser>,
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum GithubLabelValue {
    Name(String),
    Label(Box<GithubLabel>),
}

impl GithubValue for GithubLabelValue {
    fn read(value: &Value) -> Option<Self> {
        match value {
            Value::String(name) => Some(Self::Name(name.clone())),
            Value::Object(_) => GithubLabel::read(value).map(|label| Self::Label(Box::new(label))),
            _ => None,
        }
    }
}

model!(GithubReactions {
    url: Option<String>, total_count: Option<u64>,
    #[serde(rename = "+1")]
    plus_one: Option<u64> = |v: &Value| v["+1"].as_u64(),
    #[serde(rename = "-1")]
    minus_one: Option<u64> = |v: &Value| v["-1"].as_u64(),
    laugh: Option<u64>, hooray: Option<u64>, confused: Option<u64>,
    heart: Option<u64>, rocket: Option<u64>, eyes: Option<u64>,
});

model!(GithubIssue {
    id: Option<u64>, node_id: Option<String>, number: Option<u64>,
    title: Option<String>, body: Option<String> = |v: &Value| body(&v["body"]),
    state: Option<String>, state_reason: Option<String>, html_url: Option<String>,
    user: Option<GithubUser>, labels: Vec<GithubLabelValue>,
    assignee: Option<GithubUser>, assignees: Vec<GithubUser>, milestone: Option<GithubMilestone>,
    locked: Option<bool>, active_lock_reason: Option<String>, comments: Option<u64>,
    author_association: Option<String>, created_at: Option<String>, updated_at: Option<String>,
    closed_at: Option<String>, draft: Option<bool>, reactions: Option<GithubReactions>,
    pull_request: Option<GithubIssuePullRequest>, sub_issues_summary: Option<GithubSubIssuesSummary>,
    issue_dependencies_summary: Option<GithubIssueDependenciesSummary>,
});

model!(GithubIssuePullRequest {
    url: Option<String>, html_url: Option<String>, diff_url: Option<String>,
    patch_url: Option<String>, merged_at: Option<String>,
});

model!(GithubSubIssuesSummary {
    total: Option<u64>, completed: Option<u64>, percent_completed: Option<u64>,
});

model!(GithubIssueDependenciesSummary {
    blocked_by: Option<u64>, blocking: Option<u64>, total_blocked_by: Option<u64>, total_blocking: Option<u64>,
});

model!(GithubMilestone {
    id: Option<u64>, node_id: Option<String>, number: Option<u64>, title: Option<String>,
    description: Option<String> = |v: &Value| body(&v["description"]),
    state: Option<String>, html_url: Option<String>, creator: Option<GithubUser>,
    open_issues: Option<u64>, closed_issues: Option<u64>, due_on: Option<String>,
    created_at: Option<String>, updated_at: Option<String>, closed_at: Option<String>,
});

model!(GithubComment {
    id: Option<u64>, node_id: Option<String>,
    body: Option<String> = |v: &Value| body(&v["body"]),
    html_url: Option<String>, user: Option<GithubUser>, created_at: Option<String>,
    updated_at: Option<String>, author_association: Option<String>, reactions: Option<GithubReactions>,
    commit_id: Option<String>, original_commit_id: Option<String>,
    path: Option<String>, diff_hunk: Option<String> = |v: &Value| body(&v["diff_hunk"]),
    line: Option<u64>, original_line: Option<u64>, side: Option<String>,
    start_line: Option<u64>, original_start_line: Option<u64>, start_side: Option<String>,
    position: Option<u64>, original_position: Option<u64>, subject_type: Option<String>,
    pull_request_review_id: Option<u64>, in_reply_to_id: Option<u64>,
    discussion_id: Option<u64>, parent_id: Option<u64>, child_comment_count: Option<u64>,
    url: Option<String>, issue_url: Option<String>, pull_request_url: Option<String>,
});

model!(GithubPullRequest [issue: GithubIssue] {
    merged: Option<bool>,
    head: Option<GithubBranch>, base: Option<GithubBranch>,
    merged_at: Option<String>, merged_by: Option<GithubUser>, merge_commit_sha: Option<String>,
    mergeable: Option<bool>, mergeable_state: Option<String>, rebaseable: Option<bool>,
    additions: Option<u64>, deletions: Option<u64>, changed_files: Option<u64>, commits: Option<u64>,
    review_comments: Option<u64>, requested_reviewers: Vec<GithubUser>, requested_teams: Vec<GithubTeam>,
    maintainer_can_modify: Option<bool>, diff_url: Option<String>, patch_url: Option<String>,
});

model!(GithubBranch {
    label: Option<String>,
    #[serde(rename = "ref")]
    ref_name: Option<String> = |v: &Value| String::read(&v["ref"]),
    sha: Option<String>, user: Option<GithubUser>, repo: Option<GithubRepository>,
});

model!(GithubTeam {
    id: Option<u64>, node_id: Option<String>, name: Option<String>, slug: Option<String>,
    description: Option<String>, html_url: Option<String>, privacy: Option<String>,
});

model!(GithubReview {
    id: Option<u64>, node_id: Option<String>, state: Option<String>,
    body: Option<String> = |v: &Value| body(&v["body"]),
    user: Option<GithubUser>, commit_id: Option<String>, html_url: Option<String>,
    submitted_at: Option<String>, updated_at: Option<String>, author_association: Option<String>,
    pull_request_url: Option<String>,
});

model!(GithubReviewThread {
    node_id: Option<String>, comments: Vec<GithubComment>,
});

model!(GithubRepository {
    id: Option<u64>, node_id: Option<String>, name: Option<String>, full_name: Option<String>,
    html_url: Option<String>, owner: Option<GithubUser>,
    description: Option<String> = |v: &Value| body(&v["description"]),
    private: Option<bool>, visibility: Option<String>, default_branch: Option<String>,
    archived: Option<bool>, disabled: Option<bool>, fork: Option<bool>, is_template: Option<bool>,
    topics: Vec<String>, language: Option<String>, homepage: Option<String>,
    created_at: Option<GithubTimestamp>, updated_at: Option<GithubTimestamp>, pushed_at: Option<GithubTimestamp>,
    stargazers_count: Option<u64>, watchers_count: Option<u64>, forks_count: Option<u64>,
    open_issues_count: Option<u64>, size: Option<u64>,
    has_issues: Option<bool>, has_pull_requests: Option<bool>, has_discussions: Option<bool>,
    has_projects: Option<bool>, has_wiki: Option<bool>, has_pages: Option<bool>,
    allow_forking: Option<bool>, web_commit_signoff_required: Option<bool>,
    clone_url: Option<String>, ssh_url: Option<String>, license: Option<GithubLicense>,
});

model!(GithubLicense {
    key: Option<String>, name: Option<String>, spdx_id: Option<String>, url: Option<String>, node_id: Option<String>,
});

model!(GithubCommitAuthor {
    name: Option<String>, email: Option<String>, username: Option<String>, date: Option<String>,
});

model!(GithubCommit {
    id: Option<String>, tree_id: Option<String>,
    message: Option<String> = |v: &Value| body(&v["message"]),
    timestamp: Option<String>, url: Option<String>, distinct: Option<bool>,
    author: Option<GithubCommitAuthor>, committer: Option<GithubCommitAuthor>,
    added: Vec<String>, removed: Vec<String>, modified: Vec<String>,
});

model!(GithubStatusBranch {
    name: Option<String>, protected: Option<bool>, commit: Option<GithubCommitReference>,
});

model!(GithubCommitReference {
    sha: Option<String>, url: Option<String>, html_url: Option<String>,
});

model!(GithubStatusCommit {
    sha: Option<String>, node_id: Option<String>, url: Option<String>, html_url: Option<String>,
    author: Option<GithubUser>, committer: Option<GithubUser>, commit: Option<GithubGitCommit>,
    parents: Vec<GithubCommitReference>,
});

model!(GithubGitCommit {
    message: Option<String> = |v: &Value| body(&v["message"]),
    author: Option<GithubCommitAuthor>, committer: Option<GithubCommitAuthor>,
    tree: Option<GithubCommitReference>, url: Option<String>, comment_count: Option<u64>,
});

model!(GithubWorkflow {
    id: Option<u64>, node_id: Option<String>, name: Option<String>, path: Option<String>,
    state: Option<String>, html_url: Option<String>, badge_url: Option<String>,
    created_at: Option<String>, updated_at: Option<String>,
});

model!(GithubWorkflowRun {
    id: Option<u64>, name: Option<String>, status: Option<String>, conclusion: Option<String>,
    html_url: Option<String>, head_branch: Option<String>, head_sha: Option<String>, run_number: Option<u64>,
    node_id: Option<String>, display_title: Option<String>, event: Option<String>, path: Option<String>,
    run_attempt: Option<u64>, actor: Option<GithubUser>, triggering_actor: Option<GithubUser>,
    created_at: Option<String>, updated_at: Option<String>, run_started_at: Option<String>,
    head_commit: Option<GithubCommit>, head_repository: Option<GithubRepository>,
    pull_requests: Vec<GithubPullRequestReference>, referenced_workflows: Vec<GithubReferencedWorkflow>,
    check_suite_id: Option<u64>, jobs_url: Option<String>, logs_url: Option<String>,
    artifacts_url: Option<String>, rerun_url: Option<String>, previous_attempt_url: Option<String>,
});

model!(GithubPullRequestReference {
    id: Option<u64>, number: Option<u64>, url: Option<String>, head: Option<GithubBranch>, base: Option<GithubBranch>,
});

model!(GithubReferencedWorkflow {
    path: Option<String>, sha: Option<String>,
    #[serde(rename = "ref")]
    ref_name: Option<String> = |v: &Value| String::read(&v["ref"]),
});

model!(GithubWorkflowJob {
    id: Option<u64>, node_id: Option<String>, run_id: Option<u64>, run_attempt: Option<u64>,
    name: Option<String>, workflow_name: Option<String>, head_branch: Option<String>, head_sha: Option<String>,
    status: Option<String>, conclusion: Option<String>, html_url: Option<String>,
    created_at: Option<String>, started_at: Option<String>, completed_at: Option<String>,
    runner_id: Option<u64>, runner_name: Option<String>, runner_group_id: Option<u64>, runner_group_name: Option<String>,
    labels: Vec<String>, steps: Vec<GithubWorkflowStep>, run_url: Option<String>, check_run_url: Option<String>,
});

model!(GithubWorkflowStep {
    name: Option<String>, number: Option<u64>, status: Option<String>, conclusion: Option<String>,
    started_at: Option<String>, completed_at: Option<String>,
});

model!(GithubCheckRun {
    id: Option<u64>, node_id: Option<String>, name: Option<String>, head_sha: Option<String>,
    status: Option<String>, conclusion: Option<String>, external_id: Option<String>,
    html_url: Option<String>, details_url: Option<String>, started_at: Option<String>, completed_at: Option<String>,
    output: Option<GithubCheckOutput>, app: Option<GithubApp>, check_suite: Option<GithubCheckSuite>,
    pull_requests: Vec<GithubPullRequestReference>,
});

model!(GithubCheckOutput {
    title: Option<String> = |v: &Value| body(&v["title"]),
    summary: Option<String> = |v: &Value| body(&v["summary"]),
    text: Option<String> = |v: &Value| body(&v["text"]),
    annotations_count: Option<u64>, annotations_url: Option<String>,
});

model!(GithubApp {
    id: Option<u64>, node_id: Option<String>, slug: Option<String>, name: Option<String>,
    description: Option<String> = |v: &Value| body(&v["description"]),
    owner: Option<GithubUser>, html_url: Option<String>, external_url: Option<String>,
    created_at: Option<String>, updated_at: Option<String>,
});

model!(GithubCheckSuite {
    id: Option<u64>, node_id: Option<String>, head_branch: Option<String>, head_sha: Option<String>,
    before: Option<String>, after: Option<String>, status: Option<String>, conclusion: Option<String>,
    app: Option<GithubApp>, head_commit: Option<GithubCommit>, pull_requests: Vec<GithubPullRequestReference>,
    created_at: Option<String>, updated_at: Option<String>, latest_check_runs_count: Option<u64>,
    check_runs_url: Option<String>, rerequestable: Option<bool>, runs_rerequestable: Option<bool>,
});

model!(GithubRelease {
    id: Option<u64>, node_id: Option<String>, tag_name: Option<String>, target_commitish: Option<String>,
    name: Option<String>, body: Option<String> = |v: &Value| body(&v["body"]),
    html_url: Option<String>, draft: Option<bool>, prerelease: Option<bool>, immutable: Option<bool>,
    author: Option<GithubUser>, created_at: Option<String>, updated_at: Option<String>, published_at: Option<String>,
    assets: Vec<GithubReleaseAsset>, tarball_url: Option<String>, zipball_url: Option<String>,
});

model!(GithubReleaseAsset {
    id: Option<u64>, node_id: Option<String>, name: Option<String>, label: Option<String>,
    content_type: Option<String>, state: Option<String>, size: Option<u64>, download_count: Option<u64>,
    browser_download_url: Option<String>, uploader: Option<GithubUser>, digest: Option<String>,
    created_at: Option<String>, updated_at: Option<String>,
});

model!(GithubDiscussion {
    id: Option<u64>, node_id: Option<String>, number: Option<u64>, title: Option<String>,
    body: Option<String> = |v: &Value| body(&v["body"]),
    html_url: Option<String>, user: Option<GithubUser>, category: Option<GithubDiscussionCategory>,
    state: Option<String>, state_reason: Option<String>, locked: Option<bool>, active_lock_reason: Option<String>,
    labels: Vec<GithubLabelValue>, comments: Option<u64>,
    answer_html_url: Option<String>, answer_chosen_at: Option<String>, answer_chosen_by: Option<GithubUser>,
    created_at: Option<String>, updated_at: Option<String>, author_association: Option<String>, reactions: Option<GithubReactions>,
});

model!(GithubDiscussionCategory {
    id: Option<u64>, node_id: Option<String>, repository_id: Option<u64>, name: Option<String>,
    description: Option<String>, slug: Option<String>, emoji: Option<String>, is_answerable: Option<bool>,
    created_at: Option<String>, updated_at: Option<String>,
});

model!(GithubDeployment {
    id: Option<u64>, node_id: Option<String>,
    #[serde(rename = "ref")]
    ref_name: Option<String> = |v: &Value| String::read(&v["ref"]),
    sha: Option<String>, task: Option<String>, environment: Option<String>, original_environment: Option<String>,
    description: Option<String> = |v: &Value| body(&v["description"]),
    creator: Option<GithubUser>, production_environment: Option<bool>, transient_environment: Option<bool>,
    created_at: Option<String>, updated_at: Option<String>, statuses_url: Option<String>,
    performed_via_github_app: Option<GithubApp>, payload: Option<Value>,
});

model!(GithubDeploymentStatus {
    id: Option<u64>, node_id: Option<String>, state: Option<String>,
    description: Option<String> = |v: &Value| body(&v["description"]),
    environment: Option<String>, environment_url: Option<String>, target_url: Option<String>, log_url: Option<String>,
    creator: Option<GithubUser>, created_at: Option<String>, updated_at: Option<String>,
    deployment_url: Option<String>, performed_via_github_app: Option<GithubApp>,
});

model!(GithubChanges {
    body: Option<GithubStringChange>, title: Option<GithubStringChange>, name: Option<GithubStringChange>,
    description: Option<GithubStringChange>, color: Option<GithubStringChange>,
    category: Option<GithubCategoryChange>, homepage: Option<GithubStringChange>, default_branch: Option<GithubStringChange>,
});

model!(GithubStringChange {
    from: Option<String> = |v: &Value| body(&v["from"]),
});

model!(GithubCategoryChange {
    from: Option<GithubDiscussionCategory>,
});
