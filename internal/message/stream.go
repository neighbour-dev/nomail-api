package message

import (
	"bufio"
	"encoding/json"
	"fmt"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/realtime"
	"time"

	"github.com/gofiber/fiber/v3"
)

const heartbeatInterval = 20 * time.Second

func (h *Handler) Stream(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	events, unsubscribe := h.realtime.Subscribe(userID)
	expiresAt, hasExpiry := middleware.CurrentAccessTokenExpiresAt(c)

	c.Set(fiber.HeaderContentType, "text/event-stream")
	c.Set(fiber.HeaderCacheControl, "no-cache")
	c.Set(fiber.HeaderConnection, "keep-alive")
	c.Set("X-Accel-Buffering", "no")

	return c.SendStreamWriter(func(writer *bufio.Writer) {
		defer unsubscribe()

		if err := writeSSEEvent(writer, realtime.Event{
			Type: "ready",
			Data: map[string]any{"connectedAt": time.Now().UTC()},
		}); err != nil {
			return
		}

		heartbeat := time.NewTicker(heartbeatInterval)
		defer heartbeat.Stop()

		var expiry <-chan time.Time
		var expiryTimer *time.Timer
		if hasExpiry {
			duration := time.Until(expiresAt)
			if duration <= 0 {
				return
			}
			expiryTimer = time.NewTimer(duration)
			expiry = expiryTimer.C
			defer expiryTimer.Stop()
		}

		for {
			select {
			case event, open := <-events:
				if !open || writeSSEEvent(writer, event) != nil {
					return
				}
			case <-heartbeat.C:
				if _, err := writer.WriteString(": heartbeat\n\n"); err != nil {
					return
				}
				if err := writer.Flush(); err != nil {
					return
				}
			case <-expiry:
				_ = writeSSEEvent(writer, realtime.Event{Type: "auth.expired", Data: map[string]any{}})
				return
			}
		}
	})
}

func writeSSEEvent(writer *bufio.Writer, event realtime.Event) error {
	data, err := json.Marshal(event.Data)
	if err != nil {
		return err
	}
	if event.ID != "" {
		if _, err := fmt.Fprintf(writer, "id: %s\n", event.ID); err != nil {
			return err
		}
	}
	if event.Type != "" {
		if _, err := fmt.Fprintf(writer, "event: %s\n", event.Type); err != nil {
			return err
		}
	}
	if _, err := fmt.Fprintf(writer, "data: %s\n\n", data); err != nil {
		return err
	}
	return writer.Flush()
}
