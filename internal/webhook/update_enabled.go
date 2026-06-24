package webhook

import (
	"errors"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
	"strconv"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

type updateEnabledRequest struct {
	Enabled *bool `json:"enabled" validate:"required"`
}

func (h *Handler) UpdateEnabled(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	id, err := strconv.ParseUint(c.Params("id"), 10, 64)
	if err != nil || id == 0 {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "invalid webhook config id")
	}

	req, err := util.BindAndValidate[updateEnabledRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	config, err := h.configs.GetByIDAndUserID(uint(id), userID)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeNotFound, "webhook config not found")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get webhook config")
	}

	if err := h.configs.UpdateEnabled(&config, *req.Enabled); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to update webhook config")
	}

	config.Enabled = *req.Enabled
	return response.Success(c, buildConfigResponse(config))
}
