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
    let Some(PlatformData::Github(github)) = &parsed.data else {
        panic!()
    };
    assert_eq!(
        github.to_message_content(&parsed.metadata, &parsed.raw.payload),
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
    assert_eq!(parsed.to_message_content().summary, "Bug");
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
    assert!(parsed.to_message_content().summary.is_empty());
    let missing_header =
        event(receive_with(Provider::Github, Authentication::None, &Vec::new(), b"{}").unwrap());
    assert!(missing_header.metadata.event_type.is_empty());
    assert_eq!(missing_header.raw.as_json(), Some(&json!({})));
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
    assert_eq!(parsed.to_message_content().summary, "CI");
}

#[test]
fn issue_comments_are_normal_and_retain_structured_detail() {
    let h = headers(&[("X-GitHub-Event", "issue_comment")]);
    let payload = br#"{"action":"created","issue":{"number":123,"title":"Bug","labels":[{"name":"bug","color":"ff0000"}]},"comment":{"body":"  Reply  ","user":{"login":"alice"}},"workflow_run":{"conclusion":"failure"}}"#;
    let parsed = event(receive_with(Provider::Github, Authentication::None, &h, payload).unwrap());
    assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
    let Some(PlatformData::Github(github)) = &parsed.data else {
        panic!()
    };
    let github = &github.payload;
    assert_eq!(github.issue.value().unwrap().number.into_value(), Some(123));
    let super::GithubCommentValue::Comment(comment) = github.comment.value().unwrap() else {
        panic!("expected comment object")
    };
    assert_eq!(comment.body.value_deref(), Some("  Reply  "));
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
    assert_eq!(encoded["raw"]["payload"]["format"], "json");
    assert_eq!(
        encoded["raw"]["payload"]["value"]["unknown"]["preserved"],
        true
    );
    assert_eq!(
        encoded["raw"]["header"][0],
        serde_json::json!({"name":"X-GitHub-Event","value":"new_event"})
    );
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
    let Some(PlatformData::Github(github)) = parsed.data else {
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
    let super::GithubCommentValue::Comment(comment) = github.comment.into_value().unwrap() else {
        panic!("expected comment object")
    };
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
    let super::GithubWorkflowValue::Workflow(workflow) = github.workflow.into_value().unwrap()
    else {
        panic!("expected workflow object")
    };
    assert_eq!(
        workflow.path.value_deref(),
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
    // Do not silently move the second step to index zero. The malformed whole
    // collection stays in raw; valid sibling properties remain modeled.
    assert!(job.steps.is_none());
    assert!(github.review.is_none());
    let issue = github.issue.into_value().unwrap();
    assert!(issue.assignees.is_none());
    assert!(issue.labels.is_none());
}

fn signed_github(kind: &str, delivery: &str, payload: &serde_json::Value) -> PlatformEvent {
    let body = serde_json::to_vec(payload).unwrap();
    let signature = format!(
        "sha256={}",
        hex::encode(sign(b"github-example-secret", &body))
    );
    let h = headers(&[
        ("X-GitHub-Event", kind),
        ("X-GitHub-Delivery", delivery),
        ("X-Hub-Signature-256", &signature),
    ]);
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::HmacSha256("github-example-secret"),
            &h,
            &body,
        )
        .unwrap(),
    );
    assert_eq!(parsed.raw.as_json(), Some(payload));
    assert!(
        parsed
            .raw
            .header
            .iter()
            .all(|h| !h.name.eq_ignore_ascii_case("x-hub-signature-256"))
    );
    assert_eq!(parse_raw(Provider::Github, &parsed.raw).ok(), parsed.data);
    assert_eq!(
        serde_json::from_value::<PlatformEvent>(serde_json::to_value(&parsed).unwrap()).unwrap(),
        parsed
    );
    assert_eq!(
        parsed.metadata.deduplication_key.as_ref().unwrap().scope,
        "github:delivery"
    );
    assert_eq!(
        parsed.metadata.deduplication_key.as_ref().unwrap().value,
        delivery
    );
    parsed
}

fn projection(event: &PlatformEvent) -> serde_json::Value {
    serde_json::to_value(&event.data).unwrap()["event"]["payload"].clone()
}

fn assert_native_values(expected: &serde_json::Value, actual: &serde_json::Value, path: &str) {
    use serde_json::Value;
    match expected {
        Value::Object(fields) => {
            assert!(actual.is_object(), "{path}: expected object, got {actual}");
            for (key, value) in fields {
                let next = format!("{path}.{key}");
                assert!(actual.get(key).is_some(), "{next}: missing modeled field");
                assert_native_values(value, &actual[key], &next);
            }
        }
        Value::Array(values) => {
            assert!(actual.is_array(), "{path}: expected array");
            assert_eq!(
                values.len(),
                actual.as_array().unwrap().len(),
                "{path}: array positions changed"
            );
            for (index, value) in values.iter().enumerate() {
                assert_native_values(value, &actual[index], &format!("{path}[{index}]"));
            }
        }
        _ => assert_eq!(expected, actual, "{path}: scalar type/value changed"),
    }
}

#[test]
fn pinned_webhook_catalog_uses_canonical_event_names_not_example_header_typos() {
    let operations = super::generated::WEBHOOK_OPERATIONS;
    assert_eq!(operations.len(), super::GITHUB_MODEL_OPERATION_COUNT);
    assert_eq!(operations.len(), 276);
    let events: std::collections::BTreeSet<_> =
        operations.iter().map(|(_, event)| *event).collect();
    assert_eq!(events.len(), super::GITHUB_MODEL_EVENT_COUNT);
    assert_eq!(events.len(), 76);
    assert!(events.contains("discussion"));
    assert!(events.contains("projects_v2"));
    assert!(events.contains("projects_v2_item"));
    assert!(!events.contains("discussions"));
    assert!(!events.contains("project-v2"));
}

#[test]
fn detailed_facades_keep_their_stack_footprint_bounded() {
    let payload = std::mem::size_of::<super::GithubWebhookPayload>();
    let repository = std::mem::size_of::<super::GithubRepository>();
    let pull_request = std::mem::size_of::<super::GithubPullRequest>();
    assert!(
        payload < 64 * 1024,
        "payload occupies {payload} stack bytes"
    );
    assert!(
        repository < 4096,
        "repository occupies {repository} stack bytes"
    );
    assert!(
        pull_request < 16 * 1024,
        "pull request occupies {pull_request} stack bytes"
    );
}

#[test]
fn bounded_schema_probes_preserve_documented_fields_across_all_event_families() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/operations.json")).unwrap();
    assert_eq!(fixtures.as_array().unwrap().len(), 76);
    let mut failed = Vec::new();
    for fixture in fixtures.as_array().unwrap() {
        let kind = fixture["event"].as_str().unwrap();
        if kind == "ping" {
            continue;
        } // Control ACKs are tested separately.
        let result = std::panic::catch_unwind(|| {
            let parsed =
                signed_github(kind, fixture["name"].as_str().unwrap(), &fixture["payload"]);
            let projected = projection(&parsed);
            assert_native_values(&fixture["payload"], &projected, kind);
            assert_native_values(&projected, &fixture["payload"], kind);
        });
        if result.is_err() {
            failed.push(kind);
        }
    }
    assert!(failed.is_empty(), "schema probes failed for: {failed:?}");
}

#[test]
fn sanitized_official_examples_keep_raw_and_do_not_invent_projected_values() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/official.json")).unwrap();
    assert_eq!(fixtures.as_array().unwrap().len(), 6);
    for fixture in fixtures.as_array().unwrap() {
        let kind = fixture["event"].as_str().unwrap();
        let body = serde_json::to_vec(&fixture["payload"]).unwrap();
        if kind == "ping" {
            let signature = format!(
                "sha256={}",
                hex::encode(sign(b"github-example-secret", &body))
            );
            let h = headers(&[
                ("X-GitHub-Event", "ping"),
                ("X-Hub-Signature-256", &signature),
            ]);
            assert_eq!(
                response(
                    receive_with(
                        Provider::Github,
                        Authentication::HmacSha256("github-example-secret"),
                        &h,
                        &body
                    )
                    .unwrap()
                )
                .body,
                b"pong"
            );
            continue;
        }
        let parsed = signed_github(kind, fixture["name"].as_str().unwrap(), &fixture["payload"]);
        // Public examples can contain historical fields absent from the current
        // webhook schema. All raw fields survive; projected fields must be exact.
        let projected = projection(&parsed);
        assert_native_values(&projected, &fixture["payload"], kind);
        assert_eq!(projected["action"], fixture["payload"]["action"]);
        assert_eq!(
            projected["repository"]["url"],
            fixture["payload"]["repository"]["url"]
        );
    }
}

#[test]
fn native_pr_repository_and_installation_details_have_no_details_json_wrapper() {
    let payload = json!({
        "action":"opened", "installation":{"id":1,"permissions":{"contents":"read","pull_requests":"write"},"events":["pull_request"],"repository_selection":"selected"},
        "organization":{"id":2,"login":"example","members_url":"https://example.invalid/members","public_members_url":"https://example.invalid/public-members"},
        "repository":{"id":3,"full_name":"example/repo","url":"https://example.invalid/repo",
            "permissions":{"admin":false,"pull":true,"push":false}, "allow_auto_merge":false,
            "custom_properties":{"environment":"preview","flags":[false,0,null]},
            "security_and_analysis":{"secret_scanning":{"status":"enabled"}}},
        "pull_request":{"number":4,"title":"  UI  ","merge_commit_sha":null,
            "auto_merge":{"enabled_by":{"id":5,"login":"actor"},"merge_method":"squash","commit_title":"","commit_message":"  Keep whitespace\n"},
            "_links":{"self":{"href":"https://example.invalid/pr/4"}},
            "head":{"ref":"feature","repo":{"id":6,"full_name":"example/fork","allow_auto_merge":true}},"base":{"ref":"main"}}
    });
    let parsed = signed_github("pull_request", "github-rich-pr", &payload);
    assert_eq!(projection(&parsed), payload);
    let Some(PlatformData::Github(webhook)) = parsed.data else {
        panic!("wrong provider")
    };
    assert_eq!(
        webhook
            .payload
            .repository
            .value()
            .unwrap()
            .details
            .allow_auto_merge
            .value(),
        Some(&false)
    );
    assert_eq!(
        webhook
            .payload
            .pull_request
            .value()
            .unwrap()
            .issue
            .title
            .value_deref(),
        Some("  UI  ")
    );
    assert!(webhook.payload.details.installation.value().is_some());
}

#[test]
fn arbitrary_dispatch_inputs_and_unknown_fields_are_not_coerced_or_reinterpreted() {
    let payload = json!({"action":"custom.action.not.in.catalog", "client_payload":{"enabled":false,"count":0,"items":[null,{"key":" value "}]},
        "inputs":{"enabled":true,"count":0,"text":"  value  ","empty":null},
        "future":{"arbitrary":true}});
    let parsed = signed_github("repository_dispatch", "github-dispatch", &payload);
    let mut expected = payload.clone();
    expected.as_object_mut().unwrap().remove("future");
    assert_eq!(projection(&parsed), expected);
    assert_eq!(parsed.metadata.event_type, "repository_dispatch");
    let malformed = signed_github(
        "workflow_job",
        "github-malformed",
        &json!({
            "workflow_job":{"id":7,"name":"Still present","labels":["linux",false,"arm64"],
                "steps":[{"name":"Build","number":1,"conclusion":{}}]},
            "installation":{"id":"wrong","permissions":{"contents":"read"}}
        }),
    );
    assert_eq!(
        projection(&malformed),
        json!({"workflow_job":{"id":7,"name":"Still present","steps":[{"name":"Build","number":1}]},
        "installation":{"permissions":{"contents":"read"}}})
    );
}

#[test]
fn deployment_review_comments_and_dispatch_workflows_preserve_text_wire_variants() {
    let review = json!({"action":"approved", "comment":"  Approved\n", "workflow_run":{"id":7}});
    let parsed = signed_github("deployment_review", "github-text-comment", &review);
    assert_eq!(projection(&parsed), review);
    let Some(PlatformData::Github(webhook)) = parsed.data else {
        panic!("wrong provider")
    };
    assert!(
        matches!(webhook.payload.comment.value(), Some(super::GithubCommentValue::Text(text)) if text == "  Approved\n")
    );
    let dispatch = json!({"ref":"main", "workflow":"  deploy.yml  ", "inputs":{"dry_run":false}});
    let parsed = signed_github("workflow_dispatch", "github-text-workflow", &dispatch);
    assert_eq!(projection(&parsed), dispatch);
    let Some(PlatformData::Github(webhook)) = parsed.data else {
        panic!("wrong provider")
    };
    assert!(
        matches!(webhook.payload.workflow.value(), Some(super::GithubWorkflowValue::Name(name)) if name == "  deploy.yml  ")
    );
}

#[test]
fn security_advisories_projects_and_typed_change_values_remain_native() {
    let payload = json!({"action":"created", "alert":{"number":1,"state":"open",
        "dependency":{"package":{"ecosystem":"npm","name":"example-package"},"manifest_path":"package.json","scope":"runtime"},
        "security_advisory":{"ghsa_id":"GHSA-example","cvss":{"score":9.25,"vector_string":null},
            "identifiers":[{"type":"CVE","value":"CVE-example"}]},
        "security_vulnerability":{"severity":"critical","vulnerable_version_range":"< 1.0.0","first_patched_version":{"identifier":"1.0.0"}}}});
    assert_eq!(
        projection(&signed_github(
            "dependabot_alert",
            "github-security",
            &payload
        )),
        payload
    );
    for value in [
        json!("  old  "),
        json!(7),
        json!({"id":"option_id","name":"Ready","color":"GREEN"}),
    ] {
        let payload = json!({"action":"edited", "projects_v2_item":{"id":9007199254740993_u64,"node_id":"PVTI_example","project_node_id":"PVT_example","content_node_id":"I_example","content_type":"Issue","archived_at":null},
            "changes":{"field_value":{"field_node_id":"PVTF_example","field_name":"Status","field_type":"single_select","from":null,"to":value}}});
        assert_eq!(
            projection(&signed_github(
                "projects_v2_item",
                "github-project-change",
                &payload
            )),
            payload
        );
    }
}

#[test]
fn shared_runtime_deliveries_keep_native_details_and_raw_without_api_queries() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/deliveries.json")).unwrap();
    for fixture in fixtures.as_array().unwrap() {
        let parsed = signed_github(
            fixture["event"].as_str().unwrap(),
            fixture["name"].as_str().unwrap(),
            &fixture["payload"],
        );
        let mut expected = fixture["payload"].clone();
        expected.as_object_mut().unwrap().remove("future_metadata");
        assert_eq!(projection(&parsed), expected, "{}", fixture["name"]);
        assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
    }
}
