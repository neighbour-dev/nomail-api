package message

import (
	"errors"
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/gofiber/fiber/v3"
	"github.com/google/uuid"
	"gorm.io/gorm"
)

const (
	defaultPage     = 1
	defaultPageSize = 20
	maxPageSize     = 100
	maxSummaryRunes = 160
)

type listResponse struct {
	Items    []listItemResponse `json:"items"`
	Total    int64              `json:"total"`
	Page     int                `json:"page"`
	PageSize int                `json:"pageSize"`
}

type listItemResponse struct {
	ID              uuid.UUID  `json:"id"`
	Title           string     `json:"title"`
	Summary         string     `json:"summary"`
	Source          string     `json:"source"`
	Level           string     `json:"level"`
	Status          string     `json:"status"`
	Pinned          bool       `json:"pinned"`
	WebhookConfigID uint       `json:"webhookConfigId"`
	CreatedAt       time.Time  `json:"createdAt"`
	ExpireAt        *time.Time `json:"expireAt"`
}

type detailResponse struct {
	ID             uuid.UUID             `json:"id"`
	Title          string                `json:"title"`
	Content        string                `json:"content"`
	Source         string                `json:"source"`
	Level          string                `json:"level"`
	Status         string                `json:"status"`
	Pinned         bool                  `json:"pinned"`
	WebhookConfig  webhookConfigResponse `json:"webhookConfig"`
	CreatedAt      time.Time             `json:"createdAt"`
	UpdatedAt      time.Time             `json:"updatedAt"`
	ExpireAt       *time.Time            `json:"expireAt"`
	ReadAt         *time.Time            `json:"readAt"`
	URL            string                `json:"url"`
	HasRawPayload  bool                  `json:"hasRawPayload"`
	RawContentType string                `json:"rawContentType"`
}

type webhookConfigResponse struct {
	ID       uint   `json:"id"`
	Name     string `json:"name"`
	Provider string `json:"provider"`
}

type updateRequest struct {
	Status *string `json:"status"`
	Pinned *bool   `json:"pinned"`
}

type batchRequest struct {
	IDs    []string `json:"ids" validate:"required,min=1,max=100,dive,required"`
	Action string   `json:"action" validate:"required,oneof=mark_read mark_unread pin unpin delete"`
}

type unreadCountResponse struct {
	Count int64 `json:"count"`
}

type batchResponse struct {
	Affected int64 `json:"affected"`
}

func (h *Handler) List(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	page := queryInt(c, "page", defaultPage)
	pageSize := queryInt(c, "pageSize", defaultPageSize)
	if page < 1 {
		page = defaultPage
	}
	if pageSize < 1 {
		pageSize = defaultPageSize
	}
	if pageSize > maxPageSize {
		pageSize = maxPageSize
	}

	filter, err := parseFilter(c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	messages, total, err := h.messages.ListByUserID(userID, filter, (page-1)*pageSize, pageSize)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to list messages")
	}

	items := make([]listItemResponse, 0, len(messages))
	for _, message := range messages {
		items = append(items, buildListItemResponse(message))
	}

	return response.Success(c, listResponse{Items: items, Total: total, Page: page, PageSize: pageSize})
}

func (h *Handler) UnreadCount(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	count, err := h.messages.CountUnreadByUserID(userID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to count unread messages")
	}
	return response.Success(c, unreadCountResponse{Count: count})
}

func (h *Handler) Get(c fiber.Ctx) error {
	message, userID, err := h.getCurrentUserMessage(c)
	if err != nil {
		return err
	}

	detail, err := h.buildDetailResponse(message, userID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get webhook config")
	}
	return response.Success(c, detail)
}

func (h *Handler) Update(c fiber.Ctx) error {
	message, userID, err := h.getCurrentUserMessage(c)
	if err != nil {
		return err
	}

	req, err := util.BindAndValidate[updateRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}
	values, err := updateValues(req, time.Now())
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	if err := h.messages.Update(&message, values); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to update message")
	}
	updated, err := h.messages.GetByIDAndUserID(message.ID, userID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get message")
	}
	detail, err := h.buildDetailResponse(updated, userID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get webhook config")
	}
	return response.Success(c, detail)
}

func (h *Handler) Delete(c fiber.Ctx) error {
	message, _, err := h.getCurrentUserMessage(c)
	if err != nil {
		return err
	}
	if err := h.messages.Delete(&message); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to delete message")
	}
	return response.Success(c, nil)
}

func (h *Handler) Batch(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	req, err := util.BindAndValidate[batchRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}
	ids, err := parseUUIDs(req.IDs)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "invalid message id")
	}

	var affected int64
	if req.Action == "delete" {
		affected, err = h.messages.BatchDeleteByUserID(userID, ids)
	} else {
		values, valuesErr := batchUpdateValues(req.Action, time.Now())
		if valuesErr != nil {
			return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, valuesErr.Error())
		}
		affected, err = h.messages.BatchUpdateByUserID(userID, ids, values)
	}
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to update messages")
	}
	return response.Success(c, batchResponse{Affected: affected})
}

func (h *Handler) Raw(c fiber.Ctx) error {
	message, _, err := h.getCurrentUserMessage(c)
	if err != nil {
		return err
	}
	if message.RawPayload == "" {
		return response.ErrorCodeWithMsg(c, response.CodeNotFound, "raw payload not found")
	}

	contentType := message.ContentType
	if contentType == "" {
		contentType = fiber.MIMEOctetStream
	}
	c.Set(fiber.HeaderContentType, contentType)
	return c.SendString(message.RawPayload)
}

func (h *Handler) getCurrentUserMessage(c fiber.Ctx) (tables.NotifyMessage, uint, error) {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return tables.NotifyMessage{}, 0, response.ErrorCode(c, response.CodeUnauthorized)
	}
	id, err := uuid.Parse(c.Params("id"))
	if err != nil {
		return tables.NotifyMessage{}, 0, response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "invalid message id")
	}

	message, err := h.messages.GetByIDAndUserID(id, userID)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return tables.NotifyMessage{}, 0, response.ErrorCodeWithMsg(c, response.CodeNotFound, "message not found")
		}
		return tables.NotifyMessage{}, 0, response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get message")
	}
	return message, userID, nil
}

func (h *Handler) buildDetailResponse(message tables.NotifyMessage, userID uint) (detailResponse, error) {
	config, err := h.configs.GetByIDAndUserID(message.WebhookConfigID, userID)
	if err != nil {
		return detailResponse{}, err
	}
	return detailResponse{
		ID: message.ID, Title: message.Title, Content: message.Content, Source: message.Source,
		Level: message.Level, Status: message.Status, Pinned: message.Pinned,
		WebhookConfig: webhookConfigResponse{ID: config.ID, Name: config.Name, Provider: config.Provider},
		CreatedAt:     message.CreatedAt, UpdatedAt: message.UpdatedAt, ExpireAt: message.ExpireAt,
		ReadAt: message.ReadAt, URL: message.URL, HasRawPayload: message.RawPayload != "", RawContentType: message.ContentType,
	}, nil
}

func buildListItemResponse(message tables.NotifyMessage) listItemResponse {
	return listItemResponse{
		ID: message.ID, Title: message.Title, Summary: summarize(message.Content), Source: message.Source,
		Level: message.Level, Status: message.Status, Pinned: message.Pinned,
		WebhookConfigID: message.WebhookConfigID, CreatedAt: message.CreatedAt, ExpireAt: message.ExpireAt,
	}
}

func parseFilter(c fiber.Ctx) (repo.NotifyMessageFilter, error) {
	filter := repo.NotifyMessageFilter{
		Query:  strings.TrimSpace(c.Query("query")),
		Status: strings.TrimSpace(c.Query("status")),
		Level:  strings.TrimSpace(c.Query("level")),
	}
	if filter.Status != "" && filter.Status != "unread" && filter.Status != "read" {
		return filter, errors.New("status must be unread or read")
	}

	if value := c.Query("webhookConfigId"); value != "" {
		id, err := strconv.ParseUint(value, 10, 64)
		if err != nil || id == 0 {
			return filter, errors.New("invalid webhookConfigId")
		}
		filter.WebhookConfigID = uint(id)
	}
	if value := c.Query("pinned"); value != "" {
		pinned, err := strconv.ParseBool(value)
		if err != nil {
			return filter, errors.New("invalid pinned")
		}
		filter.Pinned = &pinned
	}
	from, err := parseTimeQuery(c.Query("from"), "from")
	if err != nil {
		return filter, err
	}
	to, err := parseTimeQuery(c.Query("to"), "to")
	if err != nil {
		return filter, err
	}
	if from != nil && to != nil && from.After(*to) {
		return filter, errors.New("from must not be after to")
	}
	filter.From = from
	filter.To = to
	return filter, nil
}

func parseTimeQuery(value, name string) (*time.Time, error) {
	if value == "" {
		return nil, nil
	}
	parsed, err := time.Parse(time.RFC3339, value)
	if err != nil {
		return nil, errors.New("invalid " + name)
	}
	return &parsed, nil
}

func updateValues(req updateRequest, now time.Time) (map[string]any, error) {
	if req.Status == nil && req.Pinned == nil {
		return nil, errors.New("status or pinned is required")
	}
	values := map[string]any{}
	if req.Status != nil {
		switch *req.Status {
		case "read":
			values["status"] = "read"
			values["read_at"] = now
		case "unread":
			values["status"] = "unread"
			values["read_at"] = nil
		default:
			return nil, errors.New("status must be unread or read")
		}
	}
	if req.Pinned != nil {
		values["pinned"] = *req.Pinned
	}
	return values, nil
}

func batchUpdateValues(action string, now time.Time) (map[string]any, error) {
	switch action {
	case "mark_read":
		return map[string]any{"status": "read", "read_at": now}, nil
	case "mark_unread":
		return map[string]any{"status": "unread", "read_at": nil}, nil
	case "pin":
		return map[string]any{"pinned": true}, nil
	case "unpin":
		return map[string]any{"pinned": false}, nil
	default:
		return nil, errors.New("unsupported batch action")
	}
}

func parseUUIDs(values []string) ([]uuid.UUID, error) {
	ids := make([]uuid.UUID, 0, len(values))
	seen := make(map[uuid.UUID]struct{}, len(values))
	for _, value := range values {
		id, err := uuid.Parse(value)
		if err != nil {
			return nil, err
		}
		if _, ok := seen[id]; ok {
			continue
		}
		seen[id] = struct{}{}
		ids = append(ids, id)
	}
	return ids, nil
}

func summarize(content string) string {
	summary := strings.Join(strings.Fields(content), " ")
	if utf8.RuneCountInString(summary) <= maxSummaryRunes {
		return summary
	}
	runes := []rune(summary)
	return string(runes[:maxSummaryRunes])
}

func queryInt(c fiber.Ctx, key string, defaultValue int) int {
	value, err := strconv.Atoi(c.Query(key))
	if err != nil {
		return defaultValue
	}
	return value
}
