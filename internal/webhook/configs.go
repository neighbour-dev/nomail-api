package webhook

import (
	"errors"
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
	"strconv"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

const (
	defaultPage     = 1
	defaultPageSize = 20
	maxPageSize     = 100
)

type configListResponse struct {
	Items    []configResponse `json:"items"`
	Total    int64            `json:"total"`
	Page     int              `json:"page"`
	PageSize int              `json:"pageSize"`
}

type updateConfigRequest struct {
	Name        string `json:"name" validate:"omitempty,max=100"`
	Provider    string `json:"provider" validate:"omitempty,max=32"`
	Source      string `json:"source" validate:"omitempty,max=255"`
	Secret      string `json:"secret" validate:"omitempty,max=500"`
	Description string `json:"description" validate:"omitempty,max=500"`
}

func (h *Handler) ListConfigs(c fiber.Ctx) error {
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

	configs, total, err := h.configs.ListByUserID(userID, (page-1)*pageSize, pageSize)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to list webhook configs")
	}

	items := make([]configResponse, 0, len(configs))
	for _, config := range configs {
		items = append(items, buildConfigResponse(config))
	}

	return response.Success(c, configListResponse{
		Items:    items,
		Total:    total,
		Page:     page,
		PageSize: pageSize,
	})
}

func (h *Handler) GetConfig(c fiber.Ctx) error {
	config, err := h.getCurrentUserConfig(c)
	if err != nil {
		return err
	}

	return response.Success(c, buildConfigResponse(config))
}

func (h *Handler) UpdateConfig(c fiber.Ctx) error {
	config, err := h.getCurrentUserConfig(c)
	if err != nil {
		return err
	}

	req, err := util.BindAndValidate[updateConfigRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	values := map[string]any{}
	if req.Name != "" {
		values["name"] = req.Name
	}
	if req.Provider != "" {
		provider := NormalizeProvider(req.Provider)
		if _, ok := h.registry.Get(provider); !ok {
			return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "unsupported provider")
		}
		values["provider"] = provider
	}
	if req.Source != "" {
		values["source"] = req.Source
	}
	if req.Secret != "" {
		encryptedSecret, err := util.EncryptAESGCMString(req.Secret, h.appConfig.SecretEncryptionKey)
		if err != nil {
			return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to encrypt webhook secret")
		}
		values["encrypted_secret"] = encryptedSecret
	}
	if req.Description != "" {
		values["description"] = req.Description
	}

	if len(values) > 0 {
		if err := h.configs.Update(&config, values); err != nil {
			return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to update webhook config")
		}
	}

	updatedConfig, err := h.configs.GetByIDAndUserID(config.ID, config.UserID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get webhook config")
	}

	return response.Success(c, buildConfigResponse(updatedConfig))
}

func (h *Handler) DeleteConfig(c fiber.Ctx) error {
	config, err := h.getCurrentUserConfig(c)
	if err != nil {
		return err
	}

	if err := h.configs.Delete(&config); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to delete webhook config")
	}

	return response.Success(c, nil)
}

func (h *Handler) getCurrentUserConfig(c fiber.Ctx) (tables.WebhookConfig, error) {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return tables.WebhookConfig{}, response.ErrorCode(c, response.CodeUnauthorized)
	}

	id, err := strconv.ParseUint(c.Params("id"), 10, 64)
	if err != nil || id == 0 {
		return tables.WebhookConfig{}, response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "invalid webhook config id")
	}

	config, err := h.configs.GetByIDAndUserID(uint(id), userID)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return tables.WebhookConfig{}, response.ErrorCodeWithMsg(c, response.CodeNotFound, "webhook config not found")
		}
		return tables.WebhookConfig{}, response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get webhook config")
	}

	return config, nil
}

func queryInt(c fiber.Ctx, key string, defaultValue int) int {
	value, err := strconv.Atoi(c.Query(key))
	if err != nil {
		return defaultValue
	}
	return value
}
