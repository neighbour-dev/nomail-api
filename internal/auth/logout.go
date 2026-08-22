package auth

import (
	"errors"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

func (h *Handler) Logout(c fiber.Ctx) error {
	req, err := util.BindAndValidate[refreshTokenRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	session, err := h.sessions.GetValidByHash(util.HashToken(req.RefreshToken))
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCode(c, response.CodeUnauthorized)
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get refresh token")
	}

	if err := h.sessions.Revoke(session.ID); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to revoke login session")
	}

	return response.Success(c, nil)
}
