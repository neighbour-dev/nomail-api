package tables

import "gorm.io/gorm"

type WebhookConfig struct {
	gorm.Model
	UserID          uint   `gorm:"not null"`
	Key             string `gorm:"uniqueIndex;size:64"`
	Provider        string `gorm:"not null;size:32"`
	EncryptedSecret string `gorm:"size:1024"`
	Enabled         bool   `gorm:"not null;default:true"`
	Description     string `gorm:"size:500"`
}
