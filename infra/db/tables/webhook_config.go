package tables

import "gorm.io/gorm"

type WebhookConfig struct {
	gorm.Model

	UserID uint `gorm:"not null;index"`

	Name string `gorm:"not null;size:100"`

	Key string `gorm:"uniqueIndex;not null;size:64"`

	Provider string `gorm:"not null;size:32;index"`

	Source string `gorm:"size:255;index"`

	EncryptedSecret string `gorm:"type:text"`

	Enabled bool `gorm:"not null;default:true"`

	Description string `gorm:"size:500"`
}
