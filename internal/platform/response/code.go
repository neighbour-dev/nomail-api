package response

import "github.com/gofiber/fiber/v3"

type ResultCode int

const (
	CodeSuccess ResultCode = iota
	CodeInvalidRequest
	CodeUnauthorized
	CodeNotFound
	CodeConflict
	CodeInternalError
)

type Result struct {
	Code       int
	Msg        string
	HTTPStatus int
}

func (c ResultCode) Result() Result {
	switch c {
	case CodeSuccess:
		return Result{Code: 0, Msg: "success", HTTPStatus: fiber.StatusOK}
	case CodeInvalidRequest:
		return Result{Code: 400, Msg: "invalid request body", HTTPStatus: fiber.StatusBadRequest}
	case CodeUnauthorized:
		return Result{Code: 401, Msg: "unauthorized", HTTPStatus: fiber.StatusUnauthorized}
	case CodeNotFound:
		return Result{Code: 404, Msg: "not found", HTTPStatus: fiber.StatusNotFound}
	case CodeConflict:
		return Result{Code: 409, Msg: "conflict", HTTPStatus: fiber.StatusConflict}
	case CodeInternalError:
		return Result{Code: 500, Msg: "internal server error", HTTPStatus: fiber.StatusInternalServerError}
	default:
		return Result{Code: 500, Msg: "unknown error", HTTPStatus: fiber.StatusInternalServerError}
	}
}
