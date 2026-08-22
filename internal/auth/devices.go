package auth

import (
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
	"time"

	"github.com/gofiber/fiber/v3"
)

type deviceResponse struct {
	DeviceID     string     `json:"deviceId"`
	DeviceName   string     `json:"deviceName"`
	Platform     string     `json:"platform"`
	DeviceType   string     `json:"deviceType"`
	Manufacturer string     `json:"manufacturer"`
	Model        string     `json:"model"`
	OSVersion    string     `json:"osVersion"`
	AppVersion   string     `json:"appVersion"`
	AppBuild     string     `json:"appBuild"`
	Locale       string     `json:"locale"`
	TimeZone     string     `json:"timeZone"`
	IPAddress    string     `json:"ipAddress"`
	UserAgent    string     `json:"userAgent"`
	LastSeenAt   *time.Time `json:"lastSeenAt"`
	LoggedIn     bool       `json:"loggedIn"`
	PushEnabled  bool       `json:"pushEnabled"`
	ExpiresAt    *time.Time `json:"expiresAt"`
	LoggedInAt   *time.Time `json:"loggedInAt"`
	CreatedAt    time.Time  `json:"createdAt"`
	UpdatedAt    time.Time  `json:"updatedAt"`
}

type deleteDeviceParams struct {
	DeviceID string `json:"deviceId" validate:"required,max=128"`
}

func (h *Handler) Devices(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	devices, err := h.sessions.ListDevices(userID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get login devices")
	}

	return response.Success(c, buildDeviceResponses(devices))
}

func (h *Handler) DeleteDevice(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	params := deleteDeviceParams{DeviceID: c.Params("deviceId")}
	if err := util.Validate(&params); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	found, err := h.sessions.DeleteDevice(userID, params.DeviceID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to delete device")
	}
	if !found {
		return response.ErrorCodeWithMsg(c, response.CodeNotFound, "device not found")
	}

	return response.Success(c, nil)
}

func (h *Handler) KickDevice(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	params := deleteDeviceParams{DeviceID: c.Params("deviceId")}
	if err := util.Validate(&params); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	found, err := h.sessions.RevokeByDevice(userID, params.DeviceID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to revoke device session")
	}
	if !found {
		return response.ErrorCodeWithMsg(c, response.CodeNotFound, "device not found")
	}

	return response.Success(c, nil)
}

func buildDeviceResponses(devices []repo.LoginDevice) []deviceResponse {
	items := make([]deviceResponse, 0, len(devices))
	for _, device := range devices {
		items = append(items, deviceResponse{
			DeviceID:     device.DeviceID,
			DeviceName:   device.DeviceName,
			Platform:     device.Platform,
			DeviceType:   device.DeviceType,
			Manufacturer: device.Manufacturer,
			Model:        device.Model,
			OSVersion:    device.OSVersion,
			AppVersion:   device.AppVersion,
			AppBuild:     device.AppBuild,
			Locale:       device.Locale,
			TimeZone:     device.TimeZone,
			IPAddress:    device.IPAddress,
			UserAgent:    device.UserAgent,
			LastSeenAt:   device.LastSeenAt,
			LoggedIn:     device.LoggedIn,
			PushEnabled:  device.PushEnabled,
			ExpiresAt:    device.ExpiresAt,
			LoggedInAt:   device.LoggedInAt,
			CreatedAt:    device.CreatedAt,
			UpdatedAt:    device.UpdatedAt,
		})
	}
	return items
}
