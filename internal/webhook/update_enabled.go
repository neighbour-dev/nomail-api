package webhook

import (
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"

	"github.com/gofiber/fiber/v3"
)

type updateEnabledRequest struct {
	Enabled *bool `json:"enabled" validate:"required"`
}

func (h *Handler) UpdateEnabled(c fiber.Ctx) error {
	req, err := util.BindAndValidate[updateEnabledRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	config, err := h.getCurrentUserConfig(c)
	if err != nil {
		return err
	}

	if err := h.configs.UpdateEnabled(&config, *req.Enabled); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to update webhook config")
	}

	config.Enabled = *req.Enabled
	return response.Success(c, buildConfigResponse(config))
}
