package model

import "time"

type Message struct {
	Provider   string    `json:"provider"`
	ExternalID string    `json:"externalId"`
	EventType  string    `json:"eventType"`
	Time       time.Time `json:"time"`
	Title      string    `json:"title"`
	Content    string    `json:"content"`
	Source     string    `json:"source"`
	Level      string    `json:"level"`
	URL        string    `json:"url"`
	Challenge  string    `json:"challenge"`
	Data       any       `json:"data"`
}
