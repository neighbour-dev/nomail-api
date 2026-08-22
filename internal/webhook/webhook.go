package webhook

import (
	"errors"
	"nomail-api/config"
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
	"nomail-api/internal/realtime"
	"nomail-api/internal/webhook/model"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/gofiber/fiber/v3"
	"github.com/google/uuid"
	"gorm.io/gorm"
)

const (
	maxExternalIDRunes  = 255
	maxTitleRunes       = 255
	maxSourceRunes      = 100
	maxContentTypeRunes = 100
)

type Handler struct {
	registry  *Registry
	configs   *repo.WebhookConfigRepo
	messages  *repo.NotifyMessageRepo
	realtime  *realtime.Hub
	appConfig config.AppConfig
}

func NewHandler(registry Registry, db *gorm.DB, appConfig config.AppConfig, realtimeHub *realtime.Hub) *Handler {
	return &Handler{
		registry:  &registry,
		configs:   repo.NewWebhookConfigRepo(db),
		messages:  repo.NewNotifyMessageRepo(db),
		realtime:  realtimeHub,
		appConfig: appConfig,
	}
}

type receiveResponse struct {
	MessageID uuid.UUID `json:"messageId"`
	Duplicate bool      `json:"duplicate"`
}

func (h *Handler) Handle(c fiber.Ctx) error {
	key := c.Params("key")

	webhookConfig, err := h.configs.GetByKey(key)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeNotFound, "webhook config not found")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get webhook config")
	}
	parser, ok := h.registry.Get(webhookConfig.Provider)
	if !ok {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "webhook provider is not configured")
	}

	secret := ""
	if webhookConfig.EncryptedSecret != "" {
		secret, err = util.DecryptAESGCMString(webhookConfig.EncryptedSecret, h.appConfig.SecretEncryptionKey)
		if err != nil {
			return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to decrypt webhook secret")
		}
	}

	body := c.Body()
	headers := c.GetReqHeaders()
	if err := parser.Verify(headers, body, secret); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeUnauthorized, "invalid webhook signature")
	}

	parsed, err := parser.Parse(headers, body)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "invalid webhook payload")
	}
	if parsed.Challenge != "" {
		return c.JSON(fiber.Map{"challenge": parsed.Challenge})
	}

	message := buildNotifyMessage(parsed, webhookConfig, body, c.Get(fiber.HeaderContentType), time.Now())
	created, err := h.messages.Create(&message)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to save webhook message")
	}
	if created {
		h.realtime.Publish(webhookConfig.UserID, realtime.Event{
			ID:   message.ID.String(),
			Type: "message.created",
			Data: messageCreatedEvent{
				ID: message.ID, Title: message.Title, Source: message.Source, Level: message.Level,
				Status: message.Status, Pinned: message.Pinned, WebhookConfigID: message.WebhookConfigID,
				CreatedAt: message.CreatedAt, ExpireAt: message.ExpireAt,
			},
		})
	}

	return response.Success(c, receiveResponse{MessageID: message.ID, Duplicate: !created})
}

type messageCreatedEvent struct {
	ID              uuid.UUID  `json:"id"`
	Title           string     `json:"title"`
	Source          string     `json:"source"`
	Level           string     `json:"level"`
	Status          string     `json:"status"`
	Pinned          bool       `json:"pinned"`
	WebhookConfigID uint       `json:"webhookConfigId"`
	CreatedAt       time.Time  `json:"createdAt"`
	ExpireAt        *time.Time `json:"expireAt"`
}

func buildNotifyMessage(parsed model.Message, config tables.WebhookConfig, body []byte, contentType string, now time.Time) tables.NotifyMessage {
	level := normalizeLevel(parsed.Level)
	expireAt := now.Add(retentionForLevel(level))
	externalID := optionalString(truncateRunes(strings.TrimSpace(parsed.ExternalID), maxExternalIDRunes))
	title := firstNonEmpty(strings.TrimSpace(parsed.Title), strings.TrimSpace(parsed.EventType), "Webhook notification")
	source := firstNonEmpty(strings.TrimSpace(parsed.Source), strings.TrimSpace(config.Source), parsed.Provider)

	return tables.NotifyMessage{
		WebhookConfigID: config.ID,
		ExternalID:      externalID,
		Source:          truncateRunes(source, maxSourceRunes),
		Title:           truncateRunes(title, maxTitleRunes),
		Content:         strings.TrimSpace(parsed.Content),
		Level:           level,
		Status:          "unread",
		URL:             strings.TrimSpace(parsed.URL),
		RawPayload:      string(body),
		ContentType:     truncateRunes(strings.TrimSpace(contentType), maxContentTypeRunes),
		ExpireAt:        &expireAt,
	}
}

func normalizeLevel(level string) string {
	switch strings.ToLower(strings.TrimSpace(level)) {
	case "critical", "alert", "error", "failed", "failure":
		return "critical"
	case "warning", "warn":
		return "warning"
	case "success", "pass", "passed", "ok":
		return "success"
	default:
		return "normal"
	}
}

func retentionForLevel(level string) time.Duration {
	switch level {
	case "critical":
		return 30 * 24 * time.Hour
	case "warning":
		return 7 * 24 * time.Hour
	default:
		return 3 * 24 * time.Hour
	}
}

func optionalString(value string) *string {
	if value == "" {
		return nil
	}
	return &value
}

func truncateRunes(value string, limit int) string {
	if utf8.RuneCountInString(value) <= limit {
		return value
	}
	return string([]rune(value)[:limit])
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}
