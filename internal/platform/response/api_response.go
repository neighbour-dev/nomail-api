package response

import "github.com/gofiber/fiber/v3"

type ApiResponse struct {
	Code int         `json:"code"`
	Msg  string      `json:"msg"`
	Data interface{} `json:"data"`
}

func Success(c fiber.Ctx, data interface{}) error {
	result := CodeSuccess.Result()
	return c.Status(result.HTTPStatus).JSON(ApiResponse{
		Code: result.Code,
		Msg:  result.Msg,
		Data: data,
	})
}

func ErrorCode(c fiber.Ctx, code ResultCode) error {
	result := code.Result()
	return c.Status(result.HTTPStatus).JSON(ApiResponse{
		Code: result.Code,
		Msg:  result.Msg,
		Data: nil,
	})
}

func ErrorCodeWithMsg(c fiber.Ctx, code ResultCode, msg string) error {
	result := code.Result()
	return c.Status(result.HTTPStatus).JSON(ApiResponse{
		Code: result.Code,
		Msg:  msg,
		Data: nil,
	})
}
