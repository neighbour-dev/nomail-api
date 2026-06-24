package repo

import (
	"nomail-api/internal/platform/db/generated"
	"nomail-api/internal/platform/db/tables"

	"gorm.io/gorm"
)

type WebhookConfigRepo struct {
	db *gorm.DB
}

func NewWebhookConfigRepo(db *gorm.DB) *WebhookConfigRepo {
	return &WebhookConfigRepo{db: db}
}

func (r *WebhookConfigRepo) Create(config *tables.WebhookConfig) error {
	return r.db.Create(config).Error
}

func (r *WebhookConfigRepo) GetByKey(key string) (tables.WebhookConfig, error) {
	var config tables.WebhookConfig
	err := r.db.Where(
		generated.WebhookConfig.Key.Eq(key),
		generated.WebhookConfig.Enabled.Eq(true),
	).First(&config).Error
	return config, err
}
