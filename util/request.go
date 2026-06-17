package util

import (
	"reflect"
	"strings"

	"github.com/go-playground/validator/v10"
	"github.com/gofiber/fiber/v3"
)

var validate = validator.New()

func BindAndValidate[T any](c fiber.Ctx) (T, error) {
	var req T
	if err := c.Bind().Body(&req); err != nil {
		return req, err
	}

	trimStringFields(&req)
	if err := validate.Struct(req); err != nil {
		return req, err
	}

	return req, nil
}

func trimStringFields(v any) {
	value := reflect.ValueOf(v)
	if value.Kind() != reflect.Pointer || value.IsNil() {
		return
	}

	value = value.Elem()
	if value.Kind() != reflect.Struct {
		return
	}

	for i := 0; i < value.NumField(); i++ {
		field := value.Field(i)
		if field.Kind() == reflect.String && field.CanSet() {
			field.SetString(strings.TrimSpace(field.String()))
		}
	}
}
