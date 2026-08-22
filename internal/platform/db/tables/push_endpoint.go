package tables

import (
	"time"

	"gorm.io/gorm"
)

type PushEndpoint struct {
	gorm.Model

	DeviceID uint   `gorm:"not null;uniqueIndex"`
	Device   Device `gorm:"constraint:OnDelete:CASCADE;"`

	Provider       string `gorm:"not null;size:32;index"`
	Environment    string `gorm:"not null;default:production;size:32"`
	EncryptedToken string `gorm:"not null;type:text"`
	TokenHash      string `gorm:"not null;uniqueIndex;size:64"`
	Enabled        bool   `gorm:"not null;default:true;index"`

	RegisteredAt  time.Time `gorm:"not null"`
	InvalidatedAt *time.Time
	LastError     string `gorm:"size:500"`
}
