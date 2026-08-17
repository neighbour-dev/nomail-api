package parser

import (
	"encoding/json"
	"errors"
	"nomail-api/internal/webhook/model"
	"strings"
)

type GenericParser struct{}

func (p *GenericParser) Provider() string { return TypeGeneric }

func (p *GenericParser) Verify(headers map[string][]string, body []byte, secret string) error {
	return verifyHMACSHA256(headerValue(headers, "X-NoMail-Signature"), "sha256=", body, secret)
}

func (p *GenericParser) Parse(headers map[string][]string, body []byte) (model.Message, error) {
	if len(body) == 0 {
		return model.Message{}, errors.New("empty payload")
	}

	var payload map[string]any
	if err := json.Unmarshal(body, &payload); err != nil {
		content := strings.TrimSpace(string(body))
		if content == "" {
			return model.Message{}, errors.New("empty payload")
		}
		return model.Message{
			Provider:   TypeGeneric,
			ExternalID: headerValue(headers, "X-NoMail-Idempotency-Key"),
			Title:      firstNonEmpty(headerValue(headers, "X-NoMail-Title"), "Webhook notification"),
			Content:    content,
			Source:     headerValue(headers, "X-NoMail-Source"),
			Level:      headerValue(headers, "X-NoMail-Level"),
			URL:        headerValue(headers, "X-NoMail-URL"),
		}, nil
	}

	eventType := firstNonEmpty(nestedString(payload, "eventType"), nestedString(payload, "type"))
	content := firstNonEmpty(
		nestedString(payload, "content"),
		nestedString(payload, "message"),
		nestedString(payload, "body"),
	)
	if content == "" {
		encoded, _ := json.Marshal(payload)
		content = string(encoded)
	}

	return model.Message{
		Provider:   TypeGeneric,
		ExternalID: firstNonEmpty(headerValue(headers, "X-NoMail-Idempotency-Key"), nestedString(payload, "id"), nestedString(payload, "eventId")),
		EventType:  eventType,
		Time:       parseEventTime(firstNonEmpty(nestedString(payload, "time"), nestedString(payload, "timestamp"))),
		Title:      firstNonEmpty(nestedString(payload, "title"), eventType, "Webhook notification"),
		Content:    content,
		Source:     nestedString(payload, "source"),
		Level:      nestedString(payload, "level"),
		URL:        nestedString(payload, "url"),
		Data:       payload,
	}, nil
}
