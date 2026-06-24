package util

import (
	"errors"
	"reflect"
	"strings"

	"github.com/go-playground/locales/en"
	ut "github.com/go-playground/universal-translator"
	"github.com/go-playground/validator/v10"
	enTranslations "github.com/go-playground/validator/v10/translations/en"
	"github.com/gofiber/fiber/v3"
)

var (
	validate = validator.New()
	trans    ut.Translator
)

func init() {
	english := en.New()
	uni := ut.New(english, english)
	trans, _ = uni.GetTranslator("en")

	validate.RegisterTagNameFunc(func(field reflect.StructField) string {
		name := strings.SplitN(field.Tag.Get("json"), ",", 2)[0]
		if name == "-" {
			return ""
		}
		return name
	})

	_ = enTranslations.RegisterDefaultTranslations(validate, trans)
}

func BindAndValidate[T any](c fiber.Ctx) (T, error) {
	var req T
	if err := c.Bind().Body(&req); err != nil {
		return req, err
	}

	trimStringFields(&req)
	if err := validate.Struct(req); err != nil {
		return req, translateValidationError(err)
	}

	return req, nil
}

func translateValidationError(err error) error {
	var validationErrors validator.ValidationErrors
	if !errors.As(err, &validationErrors) || len(validationErrors) == 0 {
		return err
	}

	return errors.New(validationErrors[0].Translate(trans))
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
