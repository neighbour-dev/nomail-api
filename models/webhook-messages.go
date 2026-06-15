package models

type WebHookMessage struct {
	ID      string `json:"id"`
	Type    string `json:"type"`
	Content string `json:"content"`
}
