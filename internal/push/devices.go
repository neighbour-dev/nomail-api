package push

import (
	"errors"
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
	"time"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

type registerDeviceRequest struct {
	DeviceID    string `json:"deviceId" validate:"required,max=128"`
	Provider    string `json:"provider" validate:"required,oneof=apns fcm"`
	Environment string `json:"environment" validate:"omitempty,oneof=sandbox production"`
	PushToken   string `json:"pushToken" validate:"required,max=8192"`
}

type updateDeviceRequest struct {
	Provider    string `json:"provider" validate:"omitempty,oneof=apns fcm"`
	Environment string `json:"environment" validate:"omitempty,oneof=sandbox production"`
	PushToken   string `json:"pushToken" validate:"omitempty,max=8192"`
	Enabled     *bool  `json:"enabled"`
}

type deviceParams struct {
	DeviceID string `json:"deviceId" validate:"required,max=128"`
}

type endpointResponse struct {
	DeviceID      string     `json:"deviceId"`
	Provider      string     `json:"provider"`
	Environment   string     `json:"environment"`
	Enabled       bool       `json:"enabled"`
	RegisteredAt  time.Time  `json:"registeredAt"`
	InvalidatedAt *time.Time `json:"invalidatedAt"`
	LastError     string     `json:"lastError"`
}

func (h *Handler) Register(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	req, err := util.BindAndValidate[registerDeviceRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}
	if req.Environment == "" {
		req.Environment = "production"
	}

	encryptedToken, err := util.EncryptAESGCMString(req.PushToken, h.appConfig.SecretEncryptionKey)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to encrypt push token")
	}

	endpoint := tables.PushEndpoint{
		Provider:       req.Provider,
		Environment:    req.Environment,
		EncryptedToken: encryptedToken,
		TokenHash:      util.HashToken(req.PushToken),
	}
	if err := h.endpoints.Upsert(userID, req.DeviceID, &endpoint); err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeNotFound, "device not found")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to register push endpoint")
	}

	return response.Success(c, buildEndpointResponse(req.DeviceID, endpoint))
}

func (h *Handler) Update(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	params := deviceParams{DeviceID: c.Params("deviceId")}
	if err := util.Validate(&params); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}
	req, err := util.BindAndValidate[updateDeviceRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}
	if req.Provider == "" && req.Environment == "" && req.PushToken == "" && req.Enabled == nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "at least one field is required")
	}

	values := make(map[string]any)
	if req.Provider != "" {
		values["provider"] = req.Provider
	}
	if req.Environment != "" {
		values["environment"] = req.Environment
	}
	if req.Enabled != nil {
		values["enabled"] = *req.Enabled
	}
	if req.PushToken != "" {
		encryptedToken, err := util.EncryptAESGCMString(req.PushToken, h.appConfig.SecretEncryptionKey)
		if err != nil {
			return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to encrypt push token")
		}
		values["encrypted_token"] = encryptedToken
		values["token_hash"] = util.HashToken(req.PushToken)
		values["registered_at"] = time.Now()
		values["invalidated_at"] = nil
		values["last_error"] = ""
	}

	endpoint, err := h.endpoints.Update(userID, params.DeviceID, values)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeNotFound, "push endpoint not found")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to update push endpoint")
	}
	return response.Success(c, buildEndpointResponse(params.DeviceID, endpoint))
}

func (h *Handler) Delete(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	params := deviceParams{DeviceID: c.Params("deviceId")}
	if err := util.Validate(&params); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	found, err := h.endpoints.Delete(userID, params.DeviceID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to delete push endpoint")
	}
	if !found {
		return response.ErrorCodeWithMsg(c, response.CodeNotFound, "push endpoint not found")
	}
	return response.Success(c, nil)
}

func buildEndpointResponse(deviceID string, endpoint tables.PushEndpoint) endpointResponse {
	return endpointResponse{
		DeviceID:      deviceID,
		Provider:      endpoint.Provider,
		Environment:   endpoint.Environment,
		Enabled:       endpoint.Enabled,
		RegisteredAt:  endpoint.RegisteredAt,
		InvalidatedAt: endpoint.InvalidatedAt,
		LastError:     endpoint.LastError,
	}
}
