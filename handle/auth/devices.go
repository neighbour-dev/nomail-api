package auth

import (
	"nomail-api/infra/db/repo"
	"nomail-api/middleware"
	"nomail-api/models/response"
	"time"

	"github.com/gofiber/fiber/v3"
)

type deviceResponse struct {
	DeviceID   string    `json:"deviceId"`
	DeviceName string    `json:"deviceName"`
	IPAddress  string    `json:"ipAddress"`
	UserAgent  string    `json:"userAgent"`
	ExpiresAt  time.Time `json:"expiresAt"`
	CreatedAt  time.Time `json:"createdAt"`
	UpdatedAt  time.Time `json:"updatedAt"`
}

func (h *Handler) Devices(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	devices, err := h.refreshTokens.ListActiveDevices(userID)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get login devices")
	}

	return response.Success(c, buildDeviceResponses(devices))
}

func buildDeviceResponses(devices []repo.ActiveDevice) []deviceResponse {
	items := make([]deviceResponse, 0, len(devices))
	for _, device := range devices {
		items = append(items, deviceResponse{
			DeviceID:   device.DeviceID,
			DeviceName: device.DeviceName,
			IPAddress:  device.IPAddress,
			UserAgent:  device.UserAgent,
			ExpiresAt:  device.ExpiresAt,
			CreatedAt:  device.CreatedAt,
			UpdatedAt:  device.UpdatedAt,
		})
	}
	return items
}
