package models

import "time"

type WebHookMessage struct {
	Provider string    `json:"provider"`
	ID       string    `json:"id"`
	Type     string    `json:"type"`
	Time     time.Time `json:"time"`
	Content  string    `json:"content"`

	Data any `json:"data"`
}
