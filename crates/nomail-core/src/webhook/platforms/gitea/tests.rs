use crate::webhook::{test_support::*, *};
use serde_json::{Value, json};

fn signature(body: &[u8]) -> String {
    hex::encode(sign(b"test-secret", body))
}

fn receive_json(kind: &str, exact: &str, payload: &Value) -> PlatformEvent {
    let body = serde_json::to_vec(payload).unwrap();
    let h = headers(&[
        ("Content-Type", "application/json"),
        ("X-Gitea-Event", kind),
        ("X-Gitea-Event-Type", exact),
        ("X-Gitea-Delivery", "delivery-1"),
        ("X-Gitea-Hook-Installation-Target-Type", "repository"),
        ("X-Gitea-Signature", &signature(&body)),
    ]);
    let parsed = event(
        receive_with(
            Provider::Gitea,
            Authentication::HmacSha256("test-secret"),
            &h,
            &body,
        )
        .unwrap(),
    );
    assert_eq!(parsed.raw.as_json(), Some(payload));
    let roundtrip: PlatformEvent =
        serde_json::from_value(serde_json::to_value(&parsed).unwrap()).unwrap();
    assert_eq!(roundtrip, parsed);
    assert_eq!(parsed.provider(), Provider::Gitea);
    assert_eq!(parsed.metadata.event_type, kind);
    assert_eq!(
        parsed.metadata.deduplication_key.as_ref().unwrap().scope,
        "gitea:delivery"
    );
    assert_eq!(
        parsed.metadata.deduplication_key.as_ref().unwrap().value,
        "delivery-1"
    );
    assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
    parsed
}

fn model(parsed: &PlatformEvent) -> &super::GiteaEvent {
    let PlatformData::Gitea(gitea) = &parsed.data else {
        panic!("not a Gitea model")
    };
    gitea
}

#[test]
fn json_authentication_requires_native_signature_and_rejects_tampering() {
    let body = br#"{"action":"opened","issue":{"title":"Test"}}"#;
    for value in ["", "sha256=00", "00", "sha1=123", &"z".repeat(64)] {
        let h = headers(&[("X-Gitea-Event", "issues"), ("X-Gitea-Signature", value)]);
        assert_eq!(
            receive_with(
                Provider::Gitea,
                Authentication::HmacSha256("test-secret"),
                &h,
                body
            )
            .unwrap_err(),
            WebhookError::Unauthorized
        );
    }
    let mut h = headers(&[
        ("x-gitea-event", "issues"),
        ("x-gitea-signature", &signature(body)),
    ]);
    assert!(
        receive_with(
            Provider::Gitea,
            Authentication::HmacSha256("test-secret"),
            &h,
            body
        )
        .is_ok()
    );
    assert_eq!(
        receive_with(
            Provider::Gitea,
            Authentication::HmacSha256("wrong"),
            &h,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    let mut tampered = body.to_vec();
    tampered.push(b' ');
    assert_eq!(
        receive_with(
            Provider::Gitea,
            Authentication::HmacSha256("test-secret"),
            &h,
            &tampered
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    h.push(("X-Gitea-Signature".into(), signature(body)));
    assert_eq!(
        receive_with(
            Provider::Gitea,
            Authentication::HmacSha256("test-secret"),
            &h,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    let h = headers(&[
        ("X-Gitea-Event", "issues"),
        (
            "X-Hub-Signature-256",
            &format!("sha256={}", signature(body)),
        ),
        ("X-Gogs-Signature", &signature(body)),
    ]);
    assert_eq!(
        receive_with(
            Provider::Gitea,
            Authentication::HmacSha256("test-secret"),
            &h,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    assert!(matches!(
        receive_with(Provider::Gitea, Authentication::HmacSha256(""), &h, body),
        Err(WebhookError::InvalidConfiguration(_))
    ));
    assert!(matches!(
        receive_with(
            Provider::Gitea,
            Authentication::GitlabToken("test-secret"),
            &h,
            body
        ),
        Err(WebhookError::InvalidConfiguration(_))
    ));
}

#[test]
fn form_signature_uses_original_decoded_json_not_form_or_reserialized_json() {
    let raw = "{\n \"comment\": {\"body\":\"中文 🚀 & + = %\"}, \"action\": \"created\"\n}";
    let body = serde_urlencoded::to_string([("payload", raw)]).unwrap();
    let h = headers(&[
        (
            "Content-Type",
            "Application/X-Www-Form-Urlencoded; charset=utf-8",
        ),
        ("X-Gitea-Event", "issue_comment"),
        ("X-Gitea-Signature", &signature(raw.as_bytes())),
    ]);
    let parsed = event(
        receive_with(
            Provider::Gitea,
            Authentication::HmacSha256("test-secret"),
            &h,
            body.as_bytes(),
        )
        .unwrap(),
    );
    assert_eq!(
        model(&parsed).comment.as_ref().unwrap().body.as_deref(),
        Some("中文 🚀 & + = %")
    );
    assert_eq!(
        parsed.raw.as_json().unwrap(),
        &serde_json::from_str::<Value>(raw).unwrap()
    );
    for wrong in [
        signature(body.as_bytes()),
        signature(
            serde_json::from_str::<Value>(raw)
                .unwrap()
                .to_string()
                .as_bytes(),
        ),
    ] {
        let mut bad = h.clone();
        bad.retain(|(k, _)| !k.eq_ignore_ascii_case("x-gitea-signature"));
        bad.push(("X-Gitea-Signature".into(), wrong));
        assert_eq!(
            receive_with(
                Provider::Gitea,
                Authentication::HmacSha256("test-secret"),
                &bad,
                body.as_bytes()
            )
            .unwrap_err(),
            WebhookError::Unauthorized
        );
    }
    for malformed in [
        "other=value",
        "payload=%7B%7D&payload=%7B%7D",
        "payload=%5B%5D",
        "payload=invalid",
    ] {
        assert!(
            receive_with(
                Provider::Gitea,
                Authentication::None,
                &h,
                malformed.as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn issues_comments_prs_and_changes_keep_gitea_specific_fields() {
    let payload = json!({"action":"edited","number":4,"commit_id":"abc","is_pull":true,
        "repository":{"id":1,"full_name":"example/repo","stars_count":9,"website":"https://example.com","template":true,"licenses":["MIT"],"parent":{"id":2},"owner":{"id":-1,"login":"ghost"}},
        "sender":{"id":-2,"login":"actions","full_name":"Actions Bot","is_admin":false},
        "issue":{"id":3,"number":4,"title":"Test","body":"  Body\n","is_locked":true,"ref":"main","due_date":"2026-01-01T00:00:00Z","time_estimate":60,"pin_order":1,"content_version":2,"original_author":"Imported","original_author_id":5,
            "pull_request":{"merged":true,"draft":false,"html_url":"https://example.com/pr/4"},"repository":{"id":1,"owner":"example","full_name":"example/repo"},
            "assets":[{"id":6,"uuid":"synthetic-file","name":"a.png","browser_download_url":"https://example.com/file"}],"projects":[{"id":7,"title":"Board"}]},
        "comment":{"id":8,"body":"  Reply  ","original_author":"Imported","assets":[{"uuid":"synthetic-reply"}]},
        "pull_request":{"id":9,"number":4,"title":"PR","draft":true,"is_locked":true,"requested_reviewers_teams":[{"id":10,"name":"Team","units_map":{"repo.code":"write"}}],"allow_maintainer_edit":true,"merge_base":"base","head":{"ref":"feature","repo_id":1},"base":{"ref":"main"}},
        "requested_reviewer":{"id":11,"login":"reviewer"},
        "changes":{"body":{"from":"  Old body  "},"ref":{"from":"old"},"added_labels":[{"id":12,"name":"bug","exclusive":true,"is_archived":false}],"removed_labels":null},
        "future":{"untouched":true}});
    let parsed = receive_json("issues", "issues", &payload);
    let g = model(&parsed);
    assert_eq!(g.sender.as_ref().unwrap().id, Some(-2));
    assert_eq!(
        g.repository.as_ref().unwrap().owner.as_ref().unwrap().id,
        Some(-1)
    );
    assert_eq!(
        g.repository.as_ref().unwrap().parent.as_ref().unwrap().id,
        Some(2)
    );
    assert_eq!(g.repository.as_ref().unwrap().stars_count, Some(9));
    assert_eq!(g.repository.as_ref().unwrap().licenses, ["MIT"]);
    let issue = g.issue.as_ref().unwrap();
    assert_eq!(issue.is_locked, Some(true));
    assert_eq!(issue.body.as_deref(), Some("  Body\n"));
    assert_eq!(issue.pull_request.as_ref().unwrap().merged, Some(true));
    assert_eq!(
        issue.repository.as_ref().unwrap().owner.as_deref(),
        Some("example")
    );
    assert_eq!(issue.assets[0].uuid.as_deref(), Some("synthetic-file"));
    assert_eq!(issue.projects[0].id, Some(7));
    assert_eq!(issue.content_version, Some(2));
    assert_eq!(
        g.comment.as_ref().unwrap().assets[0].uuid.as_deref(),
        Some("synthetic-reply")
    );
    let pr = g.pull_request.as_ref().unwrap();
    assert_eq!(pr.allow_maintainer_edit, Some(true));
    assert_eq!(pr.head.as_ref().unwrap().repo_id, Some(1));
    assert_eq!(
        pr.requested_reviewers_teams[0].units_map["repo.code"],
        "write"
    );
    assert_eq!(g.requested_reviewer.as_ref().unwrap().id, Some(11));
    let changes = g.changes.as_ref().unwrap();
    assert_eq!(
        changes.body.as_ref().unwrap().from.as_deref(),
        Some("  Old body  ")
    );
    assert_eq!(
        changes.ref_change.as_ref().unwrap().from.as_deref(),
        Some("old")
    );
    assert_eq!(changes.added_labels[0].exclusive, Some(true));
    assert!(changes.removed_labels.is_empty());
    assert_eq!(g.is_pull, Some(true));
    assert_eq!(g.commit_id.as_deref(), Some("abc"));
}

#[test]
fn review_payload_is_type_and_content_not_github_state_and_body() {
    for (exact, kind) in [
        ("pull_request_review_approved", "pull_request_approved"),
        ("pull_request_review_rejected", "pull_request_rejected"),
        ("pull_request_review_comment", "pull_request_comment"),
    ] {
        let parsed = receive_json(
            kind,
            exact,
            &json!({"action":"reviewed","review":{"type":exact,"content":"  Review\n"},"pull_request":{"number":3,"title":"PR","html_url":"https://example.com/pr/3"}}),
        );
        let g = model(&parsed);
        assert_eq!(g.event_type.as_deref(), Some(exact));
        let review = g.review.as_ref().unwrap();
        assert_eq!(review.review_type.as_deref(), Some(exact));
        assert_eq!(review.content.as_deref(), Some("  Review\n"));
        assert_eq!(parsed.to_message_content().content, "Review");
        assert_eq!(
            parsed.to_message_content().url.as_deref(),
            Some("https://example.com/pr/3")
        );
    }
}

#[test]
fn push_ref_and_status_use_gitea_commit_and_pusher_shapes() {
    let commit = json!({"id":"abc","message":"  Commit\n","author":{"name":"Author","email":"synthetic@example.com","username":"author"},"committer":{"name":"Committer"},"verification":{"verified":true,"reason":"gpg.good","signature":"  signature\n","signer":{"username":"signer"},"payload":"  signed data\n"},"added":["a"],"removed":null,"modified":["b"]});
    let parsed = receive_json(
        "push",
        "push",
        &json!({"ref":"refs/heads/main","before":"old","after":"abc","compare_url":"https://example.com/compare","total_commits":1,"commits":[commit],"head_commit":commit,"pusher":{"id":-2,"login":"actions","full_name":"Actions Bot"}}),
    );
    let push = model(&parsed).push.as_ref().unwrap();
    assert_eq!(
        push.compare_url.as_deref(),
        Some("https://example.com/compare")
    );
    assert_eq!(push.total_commits, Some(1));
    assert_eq!(
        push.pusher.as_ref().unwrap().login.as_deref(),
        Some("actions")
    );
    assert_eq!(
        push.commits[0].author.as_ref().unwrap().username.as_deref(),
        Some("author")
    );
    let verification = push.commits[0].verification.as_ref().unwrap();
    assert_eq!(verification.verified, Some(true));
    assert_eq!(verification.payload.as_deref(), Some("  signed data\n"));
    assert_eq!(
        parsed.to_message_content().url.as_deref(),
        Some("https://example.com/compare")
    );
    let status = receive_json(
        "status",
        "status",
        &json!({"id":7,"sha":"abc","state":"failure","context":"ci","commit":commit,"target_url":"https://example.com/check"}),
    );
    assert_eq!(
        model(&status)
            .commit_status
            .as_ref()
            .unwrap()
            .commit
            .as_ref()
            .unwrap()
            .id
            .as_deref(),
        Some("abc")
    );
    assert_eq!(
        model(&status)
            .commit_status
            .as_ref()
            .unwrap()
            .commit
            .as_ref()
            .unwrap()
            .message
            .as_deref(),
        Some("  Commit\n")
    );
    let created = receive_json(
        "create",
        "create",
        &json!({"sha":"abc","ref":"main","ref_type":"branch"}),
    );
    assert_eq!(
        model(&created).git_ref.as_ref().unwrap().sha.as_deref(),
        Some("abc")
    );
    let deleted = receive_json(
        "delete",
        "delete",
        &json!({"ref":"main","ref_type":"branch","pusher_type":"user"}),
    );
    assert_eq!(
        model(&deleted)
            .git_ref
            .as_ref()
            .unwrap()
            .pusher_type
            .as_deref(),
        Some("user")
    );
}

#[test]
fn wiki_fork_package_and_release_have_typed_resources() {
    let wiki = receive_json(
        "wiki",
        "wiki",
        &json!({"action":"edited","page":"Home","comment":"  Updated wiki\n"}),
    );
    assert_eq!(
        model(&wiki).wiki.as_ref().unwrap().page.as_deref(),
        Some("Home")
    );
    assert_eq!(
        model(&wiki).wiki.as_ref().unwrap().comment.as_deref(),
        Some("  Updated wiki\n")
    );
    assert!(model(&wiki).comment.is_none());
    let fork = receive_json(
        "fork",
        "fork",
        &json!({"forkee":{"id":1,"full_name":"example/fork","html_url":"https://example.com/fork"}}),
    );
    assert_eq!(model(&fork).forkee.as_ref().unwrap().id, Some(1));
    assert_eq!(fork.to_message_content().content, "example/fork");
    let package = receive_json(
        "package",
        "package",
        &json!({"action":"created","repository":null,"package":{"id":2,"type":"container","name":"app","version":"v1","owner":{"id":3},"creator":{"id":4}},"organization":{"id":5,"name":"org","username":"org"}}),
    );
    assert_eq!(
        model(&package)
            .package
            .as_ref()
            .unwrap()
            .package_type
            .as_deref(),
        Some("container")
    );
    assert_eq!(
        model(&package)
            .organization
            .as_ref()
            .unwrap()
            .username
            .as_deref(),
        Some("org")
    );
    assert!(model(&package).repository.is_none());
    let release = receive_json(
        "release",
        "release",
        &json!({"action":"updated","release":{"id":6,"tag_name":"v1","body":"  Notes\n","draft":false,"prerelease":true,"assets":[{"id":7,"uuid":"file-id","browser_download_url":"https://example.com/file","size":10}]}}),
    );
    assert_eq!(
        model(&release).release.as_ref().unwrap().body.as_deref(),
        Some("  Notes\n")
    );
    assert_eq!(
        model(&release).release.as_ref().unwrap().assets[0]
            .uuid
            .as_deref(),
        Some("file-id")
    );
}

#[test]
fn workflow_ids_actors_completion_times_and_steps_are_typed() {
    let parsed = receive_json(
        "workflow_run",
        "workflow_run",
        &json!({"action":"completed","workflow":{"id":"ci.yml","name":"CI","deleted_at":"0001-01-01T00:00:00Z"},"workflow_run":{"id":1,"display_title":"Build PR","run_attempt":2,"trigger_actor":{"id":-2,"login":"actions"},"started_at":"2026-01-01T00:00:00Z","completed_at":"2026-01-01T00:01:00Z","repository_id":3,"pull_requests":[{"id":4,"number":5,"head":{"ref":"feature","repo":{"id":3}}}]}}),
    );
    let g = model(&parsed);
    assert_eq!(g.workflow.as_ref().unwrap().id.as_deref(), Some("ci.yml"));
    let run = g.workflow_run.as_ref().unwrap();
    assert_eq!(run.trigger_actor.as_ref().unwrap().id, Some(-2));
    assert_eq!(run.completed_at.as_deref(), Some("2026-01-01T00:01:00Z"));
    assert_eq!(
        run.pull_requests[0]
            .head
            .as_ref()
            .unwrap()
            .repo
            .as_ref()
            .unwrap()
            .id,
        Some(3)
    );
    assert_eq!(parsed.to_message_content().content, "Build PR");
    let job = receive_json(
        "workflow_job",
        "workflow_job",
        &json!({"workflow_job":{"id":6,"run_id":1,"name":"Build","status":"completed","conclusion":"failure","steps":[{"number":1,"name":"Compile","conclusion":"failure","completed_at":"2026-01-01T00:01:00Z"}]}}),
    );
    assert_eq!(
        model(&job).workflow_job.as_ref().unwrap().steps[0]
            .conclusion
            .as_deref(),
        Some("failure")
    );
}

#[test]
fn all_external_event_groups_preserve_exact_event_type_and_normal_priority() {
    // Upstream v28.3.0 AllEvents/Event; internal schedule/workflow_dispatch are
    // deliberately not claimed as separate external webhook subscriptions.
    for (exact, normalized) in [
        ("create", "create"),
        ("delete", "delete"),
        ("fork", "fork"),
        ("push", "push"),
        ("issues", "issues"),
        ("issue_assign", "issues"),
        ("issue_label", "issues"),
        ("issue_milestone", "issues"),
        ("issue_comment", "issue_comment"),
        ("pull_request", "pull_request"),
        ("pull_request_assign", "pull_request"),
        ("pull_request_label", "pull_request"),
        ("pull_request_milestone", "pull_request"),
        ("pull_request_comment", "issue_comment"),
        ("pull_request_review_approved", "pull_request_approved"),
        ("pull_request_review_rejected", "pull_request_rejected"),
        ("pull_request_review_comment", "pull_request_comment"),
        ("pull_request_sync", "pull_request"),
        ("pull_request_review_request", "pull_request"),
        ("wiki", "wiki"),
        ("repository", "repository"),
        ("release", "release"),
        ("package", "package"),
        ("status", "status"),
        ("workflow_run", "workflow_run"),
        ("workflow_job", "workflow_job"),
    ] {
        let parsed = receive_json(
            normalized,
            exact,
            &json!({"action":"future-action","future":{"keep":true}}),
        );
        assert_eq!(model(&parsed).event_type.as_deref(), Some(exact));
        assert_eq!(
            model(&parsed).installation_target_type.as_deref(),
            Some("repository")
        );
        assert!(parsed.to_message_content().title.starts_with("Gitea "));
    }
}

#[test]
fn sparse_unknown_malformed_fields_and_header_boundaries_remain_safe() {
    let payload = json!({"event_type":"forged","installation_target_type":"forged","issue":{"number":1,"labels":[false,{"id":2}],"user":false},"workflow_run":{"id":"wrong","display_title":"Kept"},"future":[1,2,3]});
    let parsed = receive_json("future_event", "future_exact", &payload);
    let g = model(&parsed);
    assert_eq!(g.event_type.as_deref(), Some("future_exact"));
    assert_eq!(
        g.workflow_run.as_ref().unwrap().display_title.as_deref(),
        Some("Kept")
    );
    assert!(g.workflow_run.as_ref().unwrap().id.is_none());
    assert_eq!(g.issue.as_ref().unwrap().labels.len(), 1);
    assert!(g.issue.as_ref().unwrap().user.is_none());
    let h = headers(&[("X-Gitea-Event", "future_event")]);
    let sparse = event(receive_with(Provider::Gitea, Authentication::None, &h, b"{}").unwrap());
    assert!(model(&sparse).event_type.is_none());
    assert!(sparse.metadata.deduplication_key.is_none());
    for bad in [b"[]".as_slice(), b"null", b"invalid"] {
        assert!(receive_with(Provider::Gitea, Authentication::None, &h, bad).is_err());
    }
    let github_headers = headers(&[("X-GitHub-Event", "issues")]);
    assert!(
        receive_with(
            Provider::Gitea,
            Authentication::None,
            &github_headers,
            b"{}"
        )
        .is_err()
    );
    let historical: super::GiteaEvent =
        serde_json::from_value(json!({"action":"opened","issue":{"number":1}})).unwrap();
    assert!(historical.issue.unwrap().assets.is_empty());
}
