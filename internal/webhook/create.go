package webhook

import (
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"

	"github.com/gofiber/fiber/v3"
)

type createConfigRequest struct {
	Name        string `json:"name" validate:"required,max=100"`
	Provider    string `json:"provider" validate:"required,max=32"`
	Source      string `json:"source" validate:"omitempty,max=255"`
	Secret      string `json:"secret" validate:"omitempty,max=500"`
	Description string `json:"description" validate:"omitempty,max=500"`
}

type configResponse struct {
	ID          uint   `json:"id"`
	Name        string `json:"name"`
	Key         string `json:"key"`
	Provider    string `json:"provider"`
	Source      string `json:"source"`
	Enabled     bool   `json:"enabled"`
	Description string `json:"description"`
	URL         string `json:"url"`
}

func (h *Handler) Create(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	req, err := util.BindAndValidate[createConfigRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	provider := NormalizeProvider(req.Provider)
	if _, ok := h.registry.Get(provider); !ok {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "unsupported provider")
	}

	encryptedSecret := ""
	if req.Secret != "" {
		encryptedSecret, err = util.EncryptAESGCMString(req.Secret, h.appConfig.SecretEncryptionKey)
		if err != nil {
			return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to encrypt webhook secret")
		}
	}

	config := tables.WebhookConfig{
		UserID:          userID,
		Name:            req.Name,
		Key:             util.RandomToken(16),
		Provider:        provider,
		Source:          req.Source,
		EncryptedSecret: encryptedSecret,
		Enabled:         true,
		Description:     req.Description,
	}

	if err := h.configs.Create(&config); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to create webhook config")
	}

	return response.Success(c, buildConfigResponse(config))
}

func buildConfigResponse(config tables.WebhookConfig) configResponse {
	return configResponse{
		ID:          config.ID,
		Name:        config.Name,
		Key:         config.Key,
		Provider:    config.Provider,
		Source:      config.Source,
		Enabled:     config.Enabled,
		Description: config.Description,
		URL:         "/api/webhook/" + config.Key,
	}
}
