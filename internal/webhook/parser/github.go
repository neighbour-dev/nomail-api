package parser

import (
	"encoding/json"
	"errors"
	"nomail-api/internal/webhook/model"
)

type GithubParser struct{}

func (p *GithubParser) Provider() string {
	return TypeGithub
}

func (p *GithubParser) Verify(headers map[string][]string, body []byte, secret string) error {
	return verifyHMACSHA256(headerValue(headers, "X-Hub-Signature-256"), "sha256=", body, secret)
}

func (p *GithubParser) Parse(headers map[string][]string, body []byte) (model.Message, error) {
	var payload map[string]any
	if err := json.Unmarshal(body, &payload); err != nil {
		return model.Message{}, errors.New("invalid JSON payload")
	}

	eventType := firstNonEmpty(headerValue(headers, "X-GitHub-Event"), "event")
	action := nestedString(payload, "action")
	repository := nestedString(payload, "repository", "full_name")
	title := "GitHub " + eventType
	if action != "" {
		title += ": " + action
	}
	if repository != "" {
		title += ": " + repository
	}

	content := firstNonEmpty(
		nestedString(payload, "head_commit", "message"),
		nestedString(payload, "workflow_run", "name"),
		nestedString(payload, "pull_request", "title"),
		nestedString(payload, "issue", "title"),
		nestedString(payload, "comment", "body"),
	)
	if content == "" {
		encoded, _ := json.Marshal(payload)
		content = string(encoded)
	}

	level := "normal"
	conclusion := nestedString(payload, "workflow_run", "conclusion")
	switch conclusion {
	case "failure", "cancelled", "timed_out", "startup_failure":
		level = "critical"
	case "success":
		level = "success"
	}

	url := firstNonEmpty(
		nestedString(payload, "workflow_run", "html_url"),
		nestedString(payload, "pull_request", "html_url"),
		nestedString(payload, "issue", "html_url"),
		nestedString(payload, "comment", "html_url"),
		nestedString(payload, "repository", "html_url"),
	)
	eventTime := firstNonEmpty(
		nestedString(payload, "head_commit", "timestamp"),
		nestedString(payload, "workflow_run", "updated_at"),
		nestedString(payload, "pull_request", "updated_at"),
		nestedString(payload, "issue", "updated_at"),
	)

	return model.Message{
		Provider:   TypeGithub,
		ExternalID: firstNonEmpty(headerValue(headers, "X-GitHub-Delivery"), nestedString(payload, "id")),
		EventType:  eventType,
		Time:       parseEventTime(eventTime),
		Title:      title,
		Content:    content,
		Source:     firstNonEmpty(repository, nestedString(payload, "sender", "login"), TypeGithub),
		Level:      level,
		URL:        url,
		Data:       payload,
	}, nil
}
