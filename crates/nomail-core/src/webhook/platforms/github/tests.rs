use crate::webhook::{test_support::*, *};
use serde_json::json;

#[test]
fn form_signature_covers_original_encoded_body() {
    let payload = json!({"action":"opened","issue":{"id":123,"title":"Bug","html_url":"https://example.com/issue"},"repository":{"full_name":"owner/repo"}});
    let body = serde_urlencoded::to_string([("payload", payload.to_string())]).unwrap();
    let signature = format!("sha256={}", hex::encode(sign(b"secret", body.as_bytes())));
    let h = headers(&[
        (
            "Content-Type",
            "application/x-www-form-urlencoded; charset=utf-8",
        ),
        ("X-GitHub-Event", "issues"),
        ("X-GitHub-Delivery", "delivery-1"),
        ("X-Hub-Signature-256", &signature),
    ]);
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::HmacSha256("secret"),
            &h,
            body.as_bytes(),
        )
        .unwrap(),
    );
    let PlatformData::Github(github) = &parsed.data else {
        panic!()
    };
    assert_eq!(
        github.to_message_content(&parsed.metadata, &parsed.raw),
        parsed.to_message_content()
    );
    assert_eq!(parsed.raw.as_json(), Some(&payload));
    assert_eq!(parsed.provider(), Provider::Github);
    assert_eq!(
        github
            .payload
            .repository
            .value()
            .unwrap()
            .full_name
            .value_deref(),
        Some("owner/repo")
    );
    assert_eq!(
        parsed.to_message_content().title,
        "GitHub issues: opened: owner/repo"
    );
    assert_eq!(parsed.to_message_content().content, "Bug");
    assert_eq!(
        parsed.to_message_content().url.as_deref(),
        Some("https://example.com/issue")
    );
    assert_eq!(
        parsed.metadata().deduplication_key.as_ref().unwrap().value,
        "delivery-1"
    );
    assert!(
        receive_with(
            Provider::Github,
            Authentication::HmacSha256("secret"),
            &h,
            payload.to_string().as_bytes()
        )
        .is_err()
    );
}

#[test]
fn control_unknown_events_and_resource_ids() {
    let h = headers(&[("X-GitHub-Event", "ping")]);
    assert_eq!(
        response(receive_with(Provider::Github, Authentication::None, &h, b"{}").unwrap()).body,
        b"pong"
    );
    let h = headers(&[("X-GitHub-Event", "future_event")]);
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::None,
            &h,
            br#"{"id":123,"custom":true}"#,
        )
        .unwrap(),
    );
    assert!(parsed.metadata().deduplication_key.is_none());
    assert!(parsed.to_message_content().content.contains("custom"));
    assert!(receive_with(Provider::Github, Authentication::None, &Vec::new(), b"{}").is_err());
}

#[test]
fn workflow_mapping() {
    let h = headers(&[("X-GitHub-Event", "workflow_run")]);
    let parsed = event(receive_with(Provider::Github, Authentication::None, &h, br#"{"repository":{"full_name":"owner/repo"},"workflow_run":{"name":"CI","conclusion":"failure","html_url":"https://example.com/run/1"}}"#).unwrap());
    assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
    assert_eq!(
        parsed.raw.as_json().unwrap()["workflow_run"]["conclusion"],
        "failure"
    );
    assert_eq!(parsed.to_message_content().content, "CI");
}

#[test]
fn issue_comments_are_normal_and_retain_structured_detail() {
    let h = headers(&[("X-GitHub-Event", "issue_comment")]);
    let payload = br#"{"action":"created","issue":{"number":123,"title":"Bug","labels":[{"name":"bug","color":"ff0000"}]},"comment":{"body":"  Reply  ","user":{"login":"alice"}},"workflow_run":{"conclusion":"failure"}}"#;
    let parsed = event(receive_with(Provider::Github, Authentication::None, &h, payload).unwrap());
    assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
    let PlatformData::Github(github) = &parsed.data else {
        panic!()
    };
    let github = &github.payload;
    assert_eq!(github.issue.value().unwrap().number.into_value(), Some(123));
    assert_eq!(
        github.comment.value().unwrap().body.value_deref(),
        Some("  Reply  ")
    );
    assert_eq!(
        serde_json::to_value(&github.issue.value().unwrap().labels.value().unwrap()[0]).unwrap()["name"],
        "bug"
    );
}

#[test]
fn event_serialization_preserves_unknown_fields() {
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::None,
            &headers(&[("X-GitHub-Event", "new_event")]),
            br#"{"unknown":{"preserved":true}}"#,
        )
        .unwrap(),
    );
    let encoded = serde_json::to_value(&parsed).unwrap();
    assert_eq!(encoded["data"]["provider"], "github");
    assert_eq!(encoded["raw"]["format"], "json");
    assert_eq!(encoded["raw"]["value"]["unknown"]["preserved"], true);
    let decoded: PlatformEvent = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, parsed);
    assert_eq!(decoded.to_message_content(), parsed.to_message_content());
}

#[test]
fn malformed_signatures_and_duplicate_form_fields_are_rejected() {
    for signature in ["", "sha256=not-hex", "sha256=00", "sha1=123"] {
        let h = headers(&[
            ("X-Hub-Signature-256", signature),
            ("X-GitHub-Event", "push"),
        ]);
        assert_eq!(
            receive_with(
                Provider::Github,
                Authentication::HmacSha256("secret"),
                &h,
                b"{}"
            )
            .unwrap_err(),
            WebhookError::Unauthorized
        );
    }
    let h = headers(&[
        ("X-GitHub-Event", "push"),
        ("Content-Type", "application/x-www-form-urlencoded"),
    ]);
    assert!(matches!(
        receive_with(
            Provider::Github,
            Authentication::None,
            &h,
            b"payload=%7B%7D&payload=%7B%7D"
        ),
        Err(WebhookError::InvalidPayload(_))
    ));
}

fn modeled(kind: &str, payload: serde_json::Value) -> super::GithubWebhookPayload {
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::None,
            &headers(&[("X-GitHub-Event", kind)]),
            &serde_json::to_vec(&payload).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(parsed.raw.as_json(), Some(&payload));
    let roundtrip: PlatformEvent =
        serde_json::from_value(serde_json::to_value(&parsed).unwrap()).unwrap();
    assert_eq!(roundtrip, parsed);
    let PlatformData::Github(github) = parsed.data else {
        panic!()
    };
    github.payload
}

#[test]
fn issue_pr_and_action_context_are_structured() {
    let github = modeled(
        "pull_request",
        json!({
            "action":"assigned", "assignee":{"id":8,"login":"reviewer","type":"User"},
            "label":{"id":9,"name":"bug","color":"ff0000","default":false},
            "milestone":{"id":10,"number":1,"title":"Next","open_issues":2,"closed_issues":3},
            "changes":{"title":{"from":"Old title"}},
            "issue":{"id":11,"number":1,"state_reason":"not_planned","pull_request":{"html_url":"https://example.com/pr/1"},"assignees":[{"id":8}],"locked":true},
            "pull_request":{"id":12,"number":2,"draft":true,"merged":false,"additions":4,"deletions":2,"changed_files":1,"commits":2,"review_comments":3,
                "head":{"ref":"feature","sha":"abc","repo":{"id":13,"owner":{"id":8}}},"base":{"ref":"main"},
                "requested_reviewers":[{"id":8}],"requested_teams":[{"id":14,"slug":"team"}],"labels":["legacy",{"id":9,"name":"bug"}]}
        }),
    );
    let issue = github.issue.into_value().unwrap();
    assert_eq!(issue.id.into_value(), Some(11));
    assert_eq!(issue.state_reason.value_deref(), Some("not_planned"));
    assert!(issue.pull_request.is_some());
    assert_eq!(issue.assignees.value().unwrap()[0].id.into_value(), Some(8));
    assert_eq!(issue.locked.into_value(), Some(true));
    let pr = github.pull_request.into_value().unwrap();
    assert_eq!(pr.issue.draft.into_value(), Some(true));
    assert_eq!(
        pr.head.value().unwrap().ref_name.value_deref(),
        Some("feature")
    );
    assert_eq!(
        pr.base.value().unwrap().ref_name.value_deref(),
        Some("main")
    );
    assert_eq!(
        pr.head
            .into_value()
            .unwrap()
            .repo
            .into_value()
            .unwrap()
            .owner
            .into_value()
            .unwrap()
            .id
            .into_value(),
        Some(8)
    );
    assert_eq!(
        (
            pr.additions.into_value(),
            pr.deletions.into_value(),
            pr.changed_files.into_value(),
            pr.commits.into_value(),
            pr.review_comments.into_value()
        ),
        (Some(4), Some(2), Some(1), Some(2), Some(3))
    );
    assert_eq!(
        pr.requested_reviewers.value().unwrap()[0].id.into_value(),
        Some(8)
    );
    assert_eq!(
        pr.requested_teams.value().unwrap()[0].slug.value_deref(),
        Some("team")
    );
    assert_eq!(
        serde_json::to_value(&pr.issue.labels.value().unwrap()[0]).unwrap(),
        "legacy"
    );
    assert_eq!(
        serde_json::to_value(&pr.issue.labels.value().unwrap()[1]).unwrap()["id"],
        9
    );
    assert_eq!(
        github
            .assignee
            .into_value()
            .unwrap()
            .user_type
            .value_deref(),
        Some("User")
    );
    assert_eq!(
        github.label.into_value().unwrap().default.into_value(),
        Some(false)
    );
    assert_eq!(
        github
            .milestone
            .into_value()
            .unwrap()
            .closed_issues
            .into_value(),
        Some(3)
    );
    assert_eq!(
        github
            .changes
            .into_value()
            .unwrap()
            .title
            .into_value()
            .unwrap()
            .from
            .value_deref(),
        Some("Old title")
    );
}

#[test]
fn reviews_preserve_code_locations_reply_threads_and_review_outcome() {
    let comment = json!({"id":20,"body":"  Review\n  ","path":"src/lib.rs","diff_hunk":"@@ -1 +1 @@\n-old\n+new\n","line":2,"original_line":1,"side":"RIGHT","start_line":1,"start_side":"RIGHT","commit_id":"abc","original_commit_id":"def","pull_request_review_id":21,"in_reply_to_id":22,"updated_at":"2026-01-02T00:00:00Z","reactions":{"+1":2,"-1":1,"total_count":3}});
    let github = modeled(
        "pull_request_review_comment",
        json!({"comment":comment,"review":{"id":21,"state":"changes_requested","body":"  Please fix  ","user":{"id":8},"submitted_at":"2026-01-01T00:00:00Z"},"thread":{"node_id":"thread-id","comments":[comment]}}),
    );
    let review = github.review.into_value().unwrap();
    assert_eq!(review.state.value_deref(), Some("changes_requested"));
    assert_eq!(review.body.value_deref(), Some("  Please fix  "));
    let comment = github.comment.into_value().unwrap();
    assert_eq!(comment.path.value_deref(), Some("src/lib.rs"));
    assert_eq!(
        (
            comment.line.into_value(),
            comment.original_line.into_value(),
            comment.start_line.into_value()
        ),
        (Some(2), Some(1), Some(1))
    );
    assert_eq!(comment.in_reply_to_id.into_value(), Some(22));
    assert_eq!(comment.pull_request_review_id.into_value(), Some(21));
    assert_eq!(comment.body.value_deref(), Some("  Review\n  "));
    assert!(comment.diff_hunk.into_value().unwrap().ends_with('\n'));
    assert_eq!(
        comment
            .reactions
            .into_value()
            .unwrap()
            .plus_one
            .into_value(),
        Some(2)
    );
    let thread = github.thread.into_value().unwrap();
    assert_eq!(thread.node_id.value_deref(), Some("thread-id"));
    assert_eq!(
        thread.comments.value().unwrap()[0].id.into_value(),
        Some(20)
    );
}

#[test]
fn workflow_jobs_steps_and_check_outputs_are_structured() {
    let github = modeled(
        "workflow_job",
        json!({
            "workflow":{"id":1,"path":".github/workflows/ci.yml"},
            "workflow_run":{"id":2,"display_title":"Build PR","run_attempt":3,"event":"pull_request","actor":{"id":4},"triggering_actor":{"id":5},"pull_requests":[{"number":6,"head":{"ref":"feature"}}]},
            "workflow_job":{"id":7,"run_id":2,"runner_name":"runner","runner_group_id":8,"labels":["linux"],"steps":[{"name":"Build","number":1,"status":"completed","conclusion":"failure","started_at":"2026-01-01T00:00:00Z"}]},
            "check_run":{"id":9,"status":"completed","conclusion":"failure","output":{"title":"Build failed","summary":"  Summary  ","text":"Details\n","annotations_count":1},"app":{"id":10,"slug":"ci","owner":{"id":4}},"check_suite":{"id":11}},
            "check_suite":{"id":11,"head_branch":"feature","conclusion":"failure","latest_check_runs_count":1}
        }),
    );
    assert_eq!(
        github.workflow.into_value().unwrap().path.value_deref(),
        Some(".github/workflows/ci.yml")
    );
    let run = github.workflow_run.into_value().unwrap();
    assert_eq!(run.run_attempt.into_value(), Some(3));
    assert_eq!(run.actor.into_value().unwrap().id.into_value(), Some(4));
    assert_eq!(
        run.triggering_actor.into_value().unwrap().id.into_value(),
        Some(5)
    );
    assert_eq!(
        run.pull_requests.value().unwrap()[0].number.into_value(),
        Some(6)
    );
    let job = github.workflow_job.into_value().unwrap();
    assert_eq!(job.run_id.into_value(), Some(2));
    assert_eq!(job.runner_group_id.into_value(), Some(8));
    assert_eq!(
        job.steps.value().unwrap()[0].conclusion.value_deref(),
        Some("failure")
    );
    let check = github.check_run.into_value().unwrap();
    assert_eq!(
        check.app.into_value().unwrap().slug.value_deref(),
        Some("ci")
    );
    assert_eq!(
        check.check_suite.into_value().unwrap().id.into_value(),
        Some(11)
    );
    assert_eq!(
        check.output.into_value().unwrap().summary.value_deref(),
        Some("  Summary  ")
    );
    assert_eq!(
        github
            .check_suite
            .into_value()
            .unwrap()
            .latest_check_runs_count
            .into_value(),
        Some(1)
    );
}

#[test]
fn releases_discussions_and_deployments_have_business_models() {
    let github = modeled(
        "release",
        json!({
            "release":{"id":1,"tag_name":"v1.0.0","body":"  Notes\n","prerelease":true,"assets":[{"id":2,"name":"app.zip","size":123,"download_count":4,"browser_download_url":"https://example.com/app.zip","uploader":{"id":3}}]},
            "discussion":{"id":4,"number":5,"title":"Question","category":{"id":6,"name":"Q&A","is_answerable":true},"state":"open","answer_chosen_by":{"id":3},"answer_html_url":"https://example.com/answer"},
            "answer":{"id":7,"body":"Answer","discussion_id":4,"parent_id":8,"child_comment_count":1},
            "old_answer":{"id":8}, "changes":{"category":{"from":{"id":9,"name":"General"}},"body":{"from":""}},
            "deployment":{"id":10,"ref":"main","environment":"preview","production_environment":false,"transient_environment":true,"creator":{"id":3},"payload":{"custom":"kept"}},
            "deployment_status":{"id":11,"state":"failure","environment_url":"https://example.com/preview","log_url":"https://example.com/log"}
        }),
    );
    let release = github.release.into_value().unwrap();
    assert_eq!(release.tag_name.value_deref(), Some("v1.0.0"));
    assert_eq!(release.body.value_deref(), Some("  Notes\n"));
    assert_eq!(
        release.assets.value().unwrap()[0]
            .download_count
            .into_value(),
        Some(4)
    );
    assert_eq!(
        release.assets.value().unwrap()[0]
            .uploader
            .value()
            .unwrap()
            .id
            .into_value(),
        Some(3)
    );
    let discussion = github.discussion.into_value().unwrap();
    assert_eq!(
        discussion
            .category
            .into_value()
            .unwrap()
            .is_answerable
            .into_value(),
        Some(true)
    );
    assert_eq!(
        discussion
            .answer_chosen_by
            .into_value()
            .unwrap()
            .id
            .into_value(),
        Some(3)
    );
    assert_eq!(
        github.answer.into_value().unwrap().parent_id.into_value(),
        Some(8)
    );
    assert_eq!(
        github.old_answer.into_value().unwrap().id.into_value(),
        Some(8)
    );
    let changes = github.changes.into_value().unwrap();
    assert_eq!(
        changes.body.into_value().unwrap().from.value_deref(),
        Some("")
    );
    assert_eq!(
        changes
            .category
            .into_value()
            .unwrap()
            .from
            .into_value()
            .unwrap()
            .name
            .value_deref(),
        Some("General")
    );
    let deployment = github.deployment.into_value().unwrap();
    assert_eq!(deployment.ref_name.value_deref(), Some("main"));
    assert_eq!(deployment.production_environment.into_value(), Some(false));
    assert_eq!(deployment.payload.into_value().unwrap()["custom"], "kept");
    assert_eq!(
        github
            .deployment_status
            .into_value()
            .unwrap()
            .state
            .value_deref(),
        Some("failure")
    );
}

#[test]
fn root_level_git_events_and_timestamp_variants_are_modeled() {
    let github = modeled(
        "push",
        json!({"ref":"refs/heads/main","before":"a","after":"b","forced":true,"deleted":false,"pusher":{"name":"actor","email":"synthetic@example.com"},"commits":[{"id":"b","message":"  Commit\n","author":{"name":"author"},"added":["new.rs"],"removed":[],"modified":["lib.rs"]}],"head_commit":null,"repository":{"created_at":1,"pushed_at":2,"updated_at":"2026-01-01T00:00:00Z"}}),
    );
    let push = &github;
    assert_eq!(push.ref_name.value_deref(), Some("refs/heads/main"));
    assert_eq!(push.forced.into_value(), Some(true));
    assert_eq!(push.head_commit, Some(None));
    assert_eq!(
        push.commits.value().unwrap()[0].message.value_deref(),
        Some("  Commit\n")
    );
    assert_eq!(
        push.commits.value().unwrap()[0]
            .added
            .value()
            .unwrap()
            .as_slice(),
        ["new.rs"]
    );
    let repository = github.repository.into_value().unwrap();
    assert_eq!(
        repository.created_at.into_value(),
        Some(super::GithubTimestamp::Unix(1))
    );
    assert!(matches!(
        repository.updated_at.into_value(),
        Some(super::GithubTimestamp::Text(_))
    ));
    let github = modeled(
        "delete",
        json!({"ref":"feature","ref_type":"branch","pusher_type":"user"}),
    );
    assert_eq!(github.ref_type.value_deref(), Some("branch"));
    assert!(github.commits.is_none());
    let github = modeled(
        "status",
        json!({"id":3,"state":"error","context":"ci","description":"Failed","sha":"b","commit":{"sha":"b","commit":{"message":"Commit","author":{"name":"author"}}},"branches":[{"name":"main","protected":false}]}),
    );
    let status = github;
    assert_eq!(status.state.value_deref(), Some("error"));
    assert_eq!(
        status
            .commit
            .into_value()
            .unwrap()
            .commit
            .into_value()
            .unwrap()
            .author
            .into_value()
            .unwrap()
            .name
            .value_deref(),
        Some("author")
    );
    assert_eq!(
        status.branches.value().unwrap()[0].protected.into_value(),
        Some(false)
    );
}

#[test]
fn malformed_new_fields_do_not_discard_siblings_or_reject_events() {
    let github = modeled(
        "workflow_job",
        json!({"issue":{"number":1,"assignees":[false,{"id":2}],"labels":[null,"legacy",{"id":3}]},"workflow_job":{"id":"wrong","name":"Still present","steps":[null,{"number":1,"name":"Step","conclusion":{"unexpected":true}}]},"review":[],"future":{"opaque":true}}),
    );
    let job = github.workflow_job.into_value().unwrap();
    assert!(job.id.is_none());
    assert_eq!(job.name.value_deref(), Some("Still present"));
    assert_eq!(job.steps.value().unwrap().len(), 1);
    assert!(job.steps.value().unwrap()[0].conclusion.is_none());
    assert!(github.review.is_none());
    let issue = github.issue.into_value().unwrap();
    assert_eq!(issue.assignees.value().unwrap().len(), 1);
    assert_eq!(issue.assignees.value().unwrap()[0].id.into_value(), Some(2));
    assert_eq!(issue.labels.value().unwrap().len(), 2);
}
