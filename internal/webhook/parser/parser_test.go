package parser

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"testing"
)

func TestGenericParserVerifyAndParse(t *testing.T) {
	body := []byte(`{"id":"event-1","title":"Disk alert","content":"Usage is 92%","level":"critical","source":"monitor"}`)
	secret := "generic-secret"
	headers := map[string][]string{
		"X-NoMail-Signature": {hmacSignature(body, secret)},
	}

	parser := &GenericParser{}
	if err := parser.Verify(headers, body, secret); err != nil {
		t.Fatalf("Verify() error = %v", err)
	}
	message, err := parser.Parse(headers, body)
	if err != nil {
		t.Fatalf("Parse() error = %v", err)
	}
	if message.ExternalID != "event-1" || message.Title != "Disk alert" || message.Level != "critical" {
		t.Fatalf("Parse() = %#v", message)
	}

	headers["X-NoMail-Signature"] = []string{"sha256=invalid"}
	if err := parser.Verify(headers, body, secret); err == nil {
		t.Fatal("Verify() expected invalid signature error")
	}
}

func TestGenericParserPlainText(t *testing.T) {
	parser := &GenericParser{}
	message, err := parser.Parse(map[string][]string{
		"X-NoMail-Title":  {"Backup finished"},
		"X-NoMail-Level":  {"success"},
		"X-NoMail-Source": {"backup"},
	}, []byte("All files uploaded"))
	if err != nil {
		t.Fatalf("Parse() error = %v", err)
	}
	if message.Title != "Backup finished" || message.Content != "All files uploaded" || message.Level != "success" {
		t.Fatalf("Parse() = %#v", message)
	}
}

func TestGithubParserVerifyAndParse(t *testing.T) {
	body := []byte(`{"repository":{"full_name":"owner/repo"},"workflow_run":{"name":"CI","conclusion":"failure","html_url":"https://example.com/run/1"}}`)
	secret := "github-secret"
	headers := map[string][]string{
		"X-Hub-Signature-256": {hmacSignature(body, secret)},
		"X-GitHub-Event":      {"workflow_run"},
		"X-GitHub-Delivery":   {"delivery-1"},
	}

	parser := &GithubParser{}
	if err := parser.Verify(headers, body, secret); err != nil {
		t.Fatalf("Verify() error = %v", err)
	}
	message, err := parser.Parse(headers, body)
	if err != nil {
		t.Fatalf("Parse() error = %v", err)
	}
	if message.ExternalID != "delivery-1" || message.Source != "owner/repo" || message.Level != "critical" {
		t.Fatalf("Parse() = %#v", message)
	}
}

func TestFeishuParserVerifyAndChallenge(t *testing.T) {
	body := []byte(`{"challenge":"challenge-value","token":"verification-token"}`)
	parser := &FeishuParser{}
	if err := parser.Verify(nil, body, "verification-token"); err != nil {
		t.Fatalf("Verify() error = %v", err)
	}
	message, err := parser.Parse(nil, body)
	if err != nil {
		t.Fatalf("Parse() error = %v", err)
	}
	if message.Challenge != "challenge-value" {
		t.Fatalf("Parse() challenge = %q", message.Challenge)
	}
}

func TestFeishuParserSignature(t *testing.T) {
	body := []byte(`{"header":{"event_id":"event-1","event_type":"im.message.receive_v1"},"event":{"message":{"content":"hello"}}}`)
	secret := "encrypt-key"
	timestamp := "1786968000"
	nonce := "nonce-value"
	digest := sha256.Sum256(append([]byte(timestamp+nonce+secret), body...))
	headers := map[string][]string{
		"X-Lark-Request-Timestamp": {timestamp},
		"X-Lark-Request-Nonce":     {nonce},
		"X-Lark-Signature":         {hex.EncodeToString(digest[:])},
	}

	parser := &FeishuParser{}
	if err := parser.Verify(headers, body, secret); err != nil {
		t.Fatalf("Verify() error = %v", err)
	}
	message, err := parser.Parse(headers, body)
	if err != nil {
		t.Fatalf("Parse() error = %v", err)
	}
	if message.ExternalID != "event-1" || message.EventType != "im.message.receive_v1" {
		t.Fatalf("Parse() = %#v", message)
	}
}

func hmacSignature(body []byte, secret string) string {
	mac := hmac.New(sha256.New, []byte(secret))
	_, _ = mac.Write(body)
	return "sha256=" + hex.EncodeToString(mac.Sum(nil))
}
