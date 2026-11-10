//! Gitea UI projections, independently modeled against the MIT-licensed upstream
//! v28.3.0 modules/structs definitions (hook, issue, pull, repo_actions, etc.).
//! No upstream code is vendored. Inbound extraction is tolerant and field-local;
//! unknown data stays in raw. Go int64 IDs include negative system-user IDs.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

trait GiteaValue: Sized {
    fn read(value: &Value) -> Option<Self>;
}

impl GiteaValue for String {
    fn read(value: &Value) -> Option<Self> {
        let text = match value {
            Value::String(s) => s.trim().to_owned(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => return None,
        };
        (!text.is_empty()).then_some(text)
    }
}

impl GiteaValue for i64 {
    fn read(value: &Value) -> Option<Self> {
        value.as_i64()
    }
}

impl GiteaValue for bool {
    fn read(value: &Value) -> Option<Self> {
        value.as_bool()
    }
}

impl<T: GiteaValue> GiteaValue for Box<T> {
    fn read(value: &Value) -> Option<Self> {
        T::read(value).map(Box::new)
    }
}

trait Field {
    fn extract(value: &Value) -> Self;
}

impl<T: GiteaValue> Field for Option<T> {
    fn extract(value: &Value) -> Self {
        T::read(value)
    }
}

impl<T: GiteaValue> Field for Vec<T> {
    fn extract(value: &Value) -> Self {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(T::read)
            .collect()
    }
}

impl Field for BTreeMap<String, String> {
    fn extract(value: &Value) -> Self {
        value
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
            .collect()
    }
}

fn verbatim(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

macro_rules! field {
    ($value:ident, $name:ident) => {
        Field::extract(&$value[stringify!($name)])
    };
    ($value:ident, $name:ident, $extract:expr) => {
        ($extract)($value)
    };
}

macro_rules! model {
    ($name:ident { $($(#[$attr:meta])* $field:ident: $ty:ty $(= $extract:expr)?),* $(,)? }) => {
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub struct $name { $($(#[$attr])* pub $field: $ty,)* }

        impl GiteaValue for $name {
            fn read(value: &Value) -> Option<Self> {
                value.as_object()?;
                Some(Self { $($field: field!(value, $field $(, $extract)?),)* })
            }
        }
    };
}

model!(GiteaEvent {
    event_type: Option<String>, installation_target_type: Option<String>,
    action: Option<String>, number: Option<i64>, commit_id: Option<String>, is_pull: Option<bool>,
    before: Option<String>, after: Option<String>,
    repository: Option<GiteaRepository>, sender: Option<GiteaUser>, organization: Option<GiteaOrganization>,
    issue: Option<GiteaIssue>, pull_request: Option<GiteaPullRequest>, comment: Option<GiteaComment>,
    requested_reviewer: Option<GiteaUser>, review: Option<GiteaReview>, changes: Option<GiteaChanges>,
    forkee: Option<GiteaRepository>, release: Option<GiteaRelease>, package: Option<GiteaPackage>,
    workflow: Option<GiteaWorkflow>, workflow_run: Option<GiteaWorkflowRun>, workflow_job: Option<GiteaWorkflowJob>,
    push: Option<GiteaPush>, git_ref: Option<GiteaRefEvent>, wiki: Option<GiteaWiki>, commit_status: Option<GiteaCommitStatus>,
});

impl GiteaEvent {
    pub(super) fn from_payload(kind: &str, payload: &Value) -> Self {
        let mut event = Self::read(payload).unwrap_or_default();
        // Populated by the receiver from native headers, not similarly named JSON fields.
        event.event_type = None;
        event.installation_target_type = None;
        match kind {
            "push" => event.push = GiteaPush::read(payload),
            "create" | "delete" => event.git_ref = GiteaRefEvent::read(payload),
            "wiki" => event.wiki = GiteaWiki::read(payload),
            "status" => event.commit_status = GiteaCommitStatus::read(payload),
            _ => {}
        }
        event
    }
}

model!(GiteaUser {
    id: Option<i64>, login: Option<String>,
    #[serde(rename = "type")]
    user_type: Option<String> = |v: &Value| String::read(&v["type"]),
    login_name: Option<String>, source_id: Option<i64>, full_name: Option<String>, email: Option<String>,
    avatar_url: Option<String>, html_url: Option<String>, language: Option<String>, is_admin: Option<bool>,
    last_login: Option<String>, created: Option<String>, restricted: Option<bool>, active: Option<bool>, prohibit_login: Option<bool>,
    location: Option<String>, website: Option<String>, description: Option<String>, visibility: Option<String>,
    followers_count: Option<i64>, following_count: Option<i64>, starred_repos_count: Option<i64>,
});

// RepositoryPayload uses User here, while Package/Workflow payloads use
// Organization. Retain the named fields of both shapes, not a GitHub user alias.
model!(GiteaOrganization {
    id: Option<i64>, name: Option<String>, username: Option<String>, login: Option<String>, full_name: Option<String>,
    email: Option<String>, avatar_url: Option<String>, html_url: Option<String>, description: Option<String>,
    website: Option<String>, location: Option<String>, visibility: Option<String>, repo_admin_change_team_access: Option<bool>,
});

model!(GiteaRepository {
    id: Option<i64>, owner: Option<GiteaUser>, name: Option<String>, full_name: Option<String>,
    description: Option<String> = |v: &Value| verbatim(&v["description"]),
    empty: Option<bool>, private: Option<bool>, fork: Option<bool>, template: Option<bool>,
    parent: Option<Box<GiteaRepository>>, mirror: Option<bool>, size: Option<i64>, language: Option<String>,
    languages_url: Option<String>, html_url: Option<String>, url: Option<String>, link: Option<String>,
    ssh_url: Option<String>, clone_url: Option<String>, original_url: Option<String>, website: Option<String>,
    stars_count: Option<i64>, forks_count: Option<i64>, watchers_count: Option<i64>, branch_count: Option<i64>,
    open_issues_count: Option<i64>, open_pr_counter: Option<i64>, release_counter: Option<i64>,
    default_branch: Option<String>, default_target_branch: Option<String>, archived: Option<bool>,
    created_at: Option<String>, updated_at: Option<String>, archived_at: Option<String>, permissions: Option<GiteaPermission>,
    has_code: Option<bool>, has_issues: Option<bool>, internal_tracker: Option<GiteaInternalTracker>,
    external_tracker: Option<GiteaExternalTracker>, has_wiki: Option<bool>, external_wiki: Option<GiteaExternalWiki>,
    has_pull_requests: Option<bool>, has_projects: Option<bool>, projects_mode: Option<String>,
    has_releases: Option<bool>, has_packages: Option<bool>, has_actions: Option<bool>, ignore_whitespace_conflicts: Option<bool>,
    allow_merge_commits: Option<bool>, allow_rebase: Option<bool>, allow_rebase_explicit: Option<bool>, allow_squash_merge: Option<bool>,
    allow_fast_forward_only_merge: Option<bool>, allow_merge_update: Option<bool>, allow_rebase_update: Option<bool>,
    allow_manual_merge: Option<bool>, autodetect_manual_merge: Option<bool>, default_delete_branch_after_merge: Option<bool>,
    default_merge_style: Option<String>, default_update_style: Option<String>, default_allow_maintainer_edit: Option<bool>,
    avatar_url: Option<String>, internal: Option<bool>, mirror_interval: Option<String>, object_format_name: Option<String>,
    mirror_updated: Option<String>, mirror_last_sync_at: Option<String>, repo_transfer: Option<GiteaRepoTransfer>,
    topics: Vec<String>, licenses: Vec<String>,
});

model!(GiteaPermission { admin: Option<bool>, push: Option<bool>, pull: Option<bool> });
model!(GiteaInternalTracker {
    enable_time_tracker: Option<bool>, allow_only_contributors_to_track_time: Option<bool>, enable_issue_dependencies: Option<bool>,
});
model!(GiteaExternalTracker {
    external_tracker_url: Option<String>, external_tracker_format: Option<String>,
    external_tracker_style: Option<String>, external_tracker_regexp_pattern: Option<String>,
});
model!(GiteaExternalWiki { external_wiki_url: Option<String> });
model!(GiteaRepoTransfer { doer: Option<GiteaUser>, recipient: Option<GiteaUser>, teams: Vec<GiteaTeam> });
model!(GiteaTeam {
    id: Option<i64>, name: Option<String>, description: Option<String>, organization: Option<GiteaOrganization>,
    includes_all_repositories: Option<bool>, permission: Option<String>, units: Vec<String>,
    units_map: BTreeMap<String, String>, can_create_org_repo: Option<bool>, visibility: Option<String>,
});

model!(GiteaIssue {
    id: Option<i64>, url: Option<String>, html_url: Option<String>, number: Option<i64>, user: Option<GiteaUser>,
    original_author: Option<String>, original_author_id: Option<i64>, title: Option<String>,
    body: Option<String> = |v: &Value| verbatim(&v["body"]),
    #[serde(rename = "ref")]
    ref_name: Option<String> = |v: &Value| String::read(&v["ref"]),
    assets: Vec<GiteaAttachment>, labels: Vec<GiteaLabel>, milestone: Option<GiteaMilestone>, projects: Vec<GiteaProject>,
    assignee: Option<GiteaUser>, assignees: Vec<GiteaUser>, state: Option<String>, is_locked: Option<bool>, comments: Option<i64>,
    created_at: Option<String>, updated_at: Option<String>, closed_at: Option<String>, due_date: Option<String>,
    time_estimate: Option<i64>, pull_request: Option<GiteaPullRequestMeta>, repository: Option<GiteaRepositoryMeta>,
    pin_order: Option<i64>, content_version: Option<i64>,
});
model!(GiteaRepositoryMeta { id: Option<i64>, name: Option<String>, owner: Option<String>, full_name: Option<String> });
model!(GiteaPullRequestMeta { merged: Option<bool>, merged_at: Option<String>, draft: Option<bool>, html_url: Option<String> });
model!(GiteaLabel {
    id: Option<i64>, name: Option<String>, exclusive: Option<bool>, is_archived: Option<bool>,
    color: Option<String>, description: Option<String>, url: Option<String>,
});
model!(GiteaMilestone {
    id: Option<i64>, title: Option<String>, description: Option<String> = |v: &Value| verbatim(&v["description"]),
    state: Option<String>, open_issues: Option<i64>, closed_issues: Option<i64>,
    created_at: Option<String>, updated_at: Option<String>, closed_at: Option<String>, due_on: Option<String>,
});
model!(GiteaProject {
    id: Option<i64>, title: Option<String>, description: Option<String> = |v: &Value| verbatim(&v["description"]),
    owner_id: Option<i64>, repo_id: Option<i64>, creator: Option<GiteaUser>, creator_id: Option<i64>,
    state: Option<String>, is_closed: Option<bool>, template_type: Option<String>, card_type: Option<String>,
    #[serde(rename = "type")]
    project_type: Option<String> = |v: &Value| String::read(&v["type"]),
    num_open_issues: Option<i64>, num_closed_issues: Option<i64>, num_issues: Option<i64>,
    created_at: Option<String>, updated_at: Option<String>, closed_at: Option<String>, html_url: Option<String>,
});

model!(GiteaPullRequest {
    id: Option<i64>, url: Option<String>, number: Option<i64>, user: Option<GiteaUser>, title: Option<String>,
    body: Option<String> = |v: &Value| verbatim(&v["body"]),
    labels: Vec<GiteaLabel>, milestone: Option<GiteaMilestone>, assignee: Option<GiteaUser>, assignees: Vec<GiteaUser>,
    requested_reviewers: Vec<GiteaUser>, requested_reviewers_teams: Vec<GiteaTeam>,
    state: Option<String>, draft: Option<bool>, is_locked: Option<bool>, comments: Option<i64>, review_comments: Option<i64>,
    additions: Option<i64>, deletions: Option<i64>, changed_files: Option<i64>, html_url: Option<String>, diff_url: Option<String>, patch_url: Option<String>,
    mergeable: Option<bool>, merged: Option<bool>, merged_at: Option<String>, merge_commit_sha: Option<String>, merged_by: Option<GiteaUser>,
    allow_maintainer_edit: Option<bool>, base: Option<GiteaBranch>, head: Option<GiteaBranch>, merge_base: Option<String>,
    due_date: Option<String>, created_at: Option<String>, updated_at: Option<String>, closed_at: Option<String>,
    pin_order: Option<i64>, content_version: Option<i64>,
});
model!(GiteaBranch {
    label: Option<String>,
    #[serde(rename = "ref")]
    ref_name: Option<String> = |v: &Value| String::read(&v["ref"]),
    sha: Option<String>, repo_id: Option<i64>, repo: Option<GiteaRepository>,
});
model!(GiteaReview {
    #[serde(rename = "type")]
    review_type: Option<String> = |v: &Value| String::read(&v["type"]),
    content: Option<String> = |v: &Value| verbatim(&v["content"]),
});
model!(GiteaChanges {
    title: Option<GiteaChange>, body: Option<GiteaChange>, name: Option<GiteaChange>,
    #[serde(rename = "ref")]
    ref_change: Option<GiteaChange> = |v: &Value| GiteaChange::read(&v["ref"]),
    added_labels: Vec<GiteaLabel>, removed_labels: Vec<GiteaLabel>,
});
model!(GiteaChange { from: Option<String> = |v: &Value| verbatim(&v["from"]) });
model!(GiteaComment {
    id: Option<i64>, html_url: Option<String>, pull_request_url: Option<String>, issue_url: Option<String>, user: Option<GiteaUser>,
    original_author: Option<String>, original_author_id: Option<i64>,
    body: Option<String> = |v: &Value| verbatim(&v["body"]),
    assets: Vec<GiteaAttachment>, created_at: Option<String>, updated_at: Option<String>,
});
model!(GiteaAttachment {
    id: Option<i64>, name: Option<String>, size: Option<i64>, download_count: Option<i64>,
    created_at: Option<String>, uuid: Option<String>, browser_download_url: Option<String>,
});

model!(GiteaPush {
    #[serde(rename = "ref")]
    ref_name: Option<String> = |v: &Value| String::read(&v["ref"]),
    before: Option<String>, after: Option<String>, compare_url: Option<String>,
    commits: Vec<GiteaCommit>, total_commits: Option<i64>, head_commit: Option<GiteaCommit>, pusher: Option<GiteaUser>,
});
model!(GiteaCommit {
    id: Option<String>, message: Option<String> = |v: &Value| verbatim(&v["message"]),
    url: Option<String>, author: Option<GiteaCommitUser>, committer: Option<GiteaCommitUser>,
    verification: Option<GiteaCommitVerification>, timestamp: Option<String>, added: Vec<String>, removed: Vec<String>, modified: Vec<String>,
});
model!(GiteaCommitUser { name: Option<String>, email: Option<String>, username: Option<String> });
model!(GiteaCommitVerification {
    verified: Option<bool>, reason: Option<String>, signature: Option<String> = |v: &Value| verbatim(&v["signature"]),
    signer: Option<GiteaCommitUser>, payload: Option<String> = |v: &Value| verbatim(&v["payload"]),
});
model!(GiteaRefEvent {
    sha: Option<String>,
    #[serde(rename = "ref")]
    ref_name: Option<String> = |v: &Value| String::read(&v["ref"]),
    ref_type: Option<String>, pusher_type: Option<String>,
});
model!(GiteaWiki {
    page: Option<String>, comment: Option<String> = |v: &Value| verbatim(&v["comment"]),
});
model!(GiteaCommitStatus {
    commit: Option<GiteaCommit>, context: Option<String>, created_at: Option<String>,
    description: Option<String> = |v: &Value| verbatim(&v["description"]),
    id: Option<i64>, sha: Option<String>, state: Option<String>, target_url: Option<String>, updated_at: Option<String>,
});

model!(GiteaRelease {
    id: Option<i64>, tag_name: Option<String>, target_commitish: Option<String>, name: Option<String>,
    body: Option<String> = |v: &Value| verbatim(&v["body"]),
    url: Option<String>, html_url: Option<String>, tarball_url: Option<String>, zipball_url: Option<String>, upload_url: Option<String>,
    draft: Option<bool>, prerelease: Option<bool>, created_at: Option<String>, published_at: Option<String>,
    author: Option<GiteaUser>, assets: Vec<GiteaAttachment>,
});
model!(GiteaPackage {
    id: Option<i64>, owner: Option<GiteaUser>, repository: Option<GiteaRepository>, creator: Option<GiteaUser>,
    #[serde(rename = "type")]
    package_type: Option<String> = |v: &Value| String::read(&v["type"]),
    name: Option<String>, version: Option<String>, html_url: Option<String>, created_at: Option<String>,
});

model!(GiteaWorkflow {
    id: Option<String>, name: Option<String>, path: Option<String>, state: Option<String>,
    created_at: Option<String>, updated_at: Option<String>, url: Option<String>, html_url: Option<String>,
    badge_url: Option<String>, deleted_at: Option<String>,
});
model!(GiteaWorkflowRun {
    id: Option<i64>, url: Option<String>, previous_attempt_url: Option<String>, html_url: Option<String>,
    jobs_url: Option<String>, logs_url: Option<String>, artifacts_url: Option<String>, cancel_url: Option<String>, rerun_url: Option<String>,
    display_title: Option<String>, path: Option<String>, event: Option<String>, run_attempt: Option<i64>, run_number: Option<i64>,
    repository_id: Option<i64>, head_sha: Option<String>, head_branch: Option<String>, status: Option<String>,
    actor: Option<GiteaUser>, trigger_actor: Option<GiteaUser>, repository: Option<GiteaRepository>, head_repository: Option<GiteaRepository>,
    conclusion: Option<String>, pull_requests: Vec<GiteaPullRequestReference>,
    created_at: Option<String>, updated_at: Option<String>, started_at: Option<String>, completed_at: Option<String>,
});
model!(GiteaPullRequestReference {
    id: Option<i64>, number: Option<i64>, url: Option<String>, head: Option<GiteaBranch>, base: Option<GiteaBranch>,
});
model!(GiteaWorkflowJob {
    id: Option<i64>, url: Option<String>, html_url: Option<String>, run_id: Option<i64>, run_url: Option<String>, name: Option<String>,
    labels: Vec<String>, run_attempt: Option<i64>, head_sha: Option<String>, head_branch: Option<String>, status: Option<String>,
    conclusion: Option<String>, runner_id: Option<i64>, runner_name: Option<String>, steps: Vec<GiteaWorkflowStep>,
    created_at: Option<String>, started_at: Option<String>, completed_at: Option<String>,
});
model!(GiteaWorkflowStep {
    name: Option<String>, number: Option<i64>, status: Option<String>, conclusion: Option<String>,
    started_at: Option<String>, completed_at: Option<String>,
});
