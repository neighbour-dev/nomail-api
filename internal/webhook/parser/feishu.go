package parser

import (
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"errors"
	"nomail-api/internal/webhook/model"
)

type FeishuParser struct{}

func (p *FeishuParser) Provider() string {
	return TypeFeiShu
}

func (p *FeishuParser) Verify(headers map[string][]string, body []byte, secret string) error {
	if secret == "" {
		return nil
	}

	signature := firstNonEmpty(headerValue(headers, "X-Lark-Signature"), headerValue(headers, "X-Feishu-Signature"))
	if signature != "" {
		timestamp := firstNonEmpty(headerValue(headers, "X-Lark-Request-Timestamp"), headerValue(headers, "X-Feishu-Request-Timestamp"))
		nonce := firstNonEmpty(headerValue(headers, "X-Lark-Request-Nonce"), headerValue(headers, "X-Feishu-Request-Nonce"))
		digest := sha256.Sum256(append([]byte(timestamp+nonce+secret), body...))
		expected := hex.EncodeToString(digest[:])
		if subtle.ConstantTimeCompare([]byte(signature), []byte(expected)) != 1 {
			return errors.New("invalid signature")
		}
		return nil
	}

	var payload map[string]any
	if err := json.Unmarshal(body, &payload); err != nil {
		return errors.New("invalid JSON payload")
	}
	token := firstNonEmpty(nestedString(payload, "token"), nestedString(payload, "header", "token"))
	if subtle.ConstantTimeCompare([]byte(token), []byte(secret)) != 1 {
		return errors.New("invalid verification token")
	}
	return nil
}

func (p *FeishuParser) Parse(headers map[string][]string, body []byte) (model.Message, error) {
	var payload map[string]any
	if err := json.Unmarshal(body, &payload); err != nil {
		return model.Message{}, errors.New("invalid JSON payload")
	}

	challenge := nestedString(payload, "challenge")
	if challenge != "" {
		return model.Message{Provider: TypeFeiShu, Challenge: challenge, Data: payload}, nil
	}

	eventType := firstNonEmpty(nestedString(payload, "header", "event_type"), nestedString(payload, "type"), "event")
	event := nestedValue(payload, "event")
	content := firstNonEmpty(
		nestedString(payload, "content"),
		nestedString(payload, "event", "message", "content"),
		nestedString(payload, "event", "content"),
	)
	if content == "" {
		encoded, _ := json.Marshal(event)
		content = string(encoded)
	}

	return model.Message{
		Provider:   TypeFeiShu,
		ExternalID: firstNonEmpty(nestedString(payload, "header", "event_id"), nestedString(payload, "uuid")),
		EventType:  eventType,
		Time:       parseEventTime(firstNonEmpty(nestedString(payload, "header", "create_time"), nestedString(payload, "ts"))),
		Title:      firstNonEmpty(nestedString(payload, "title"), eventType),
		Content:    content,
		Source:     firstNonEmpty(nestedString(payload, "source"), nestedString(payload, "header", "app_id"), TypeFeiShu),
		Level:      nestedString(payload, "level"),
		URL:        nestedString(payload, "url"),
		Data:       payload,
	}, nil
}
