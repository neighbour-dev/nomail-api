package response

import "github.com/gofiber/fiber/v3"

type ApiResponse struct {
	Code int         `json:"code"`
	Msg  string      `json:"msg"`
	Data interface{} `json:"data"`
}

func Success(c fiber.Ctx, data interface{}) error {
	return c.JSON(ApiResponse{
		Code: 0,
		Msg:  "success",
		Data: data,
	})
}

func Error(c fiber.Ctx, code int, msg string) error {
	return c.JSON(ApiResponse{
		Code: code,
		Msg:  msg,
		Data: nil,
	})
}

func ErrorWithStatus(c fiber.Ctx, httpStatus int, code int, msg string) error {
	return c.Status(httpStatus).JSON(ApiResponse{
		Code: code,
		Msg:  msg,
		Data: nil,
	})
}
