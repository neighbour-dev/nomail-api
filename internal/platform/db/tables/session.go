package tables

import (
	"time"

	"gorm.io/gorm"
)

type Session struct {
	gorm.Model

	DeviceID uint   `gorm:"not null;uniqueIndex"`
	Device   Device `gorm:"constraint:OnDelete:CASCADE;"`

	// 只存 refresh token 的 hash，不存明文 token。
	RefreshTokenHash string `gorm:"uniqueIndex;not null;size:64"`

	ExpiresAt  time.Time `gorm:"not null;index"`
	LoggedInAt time.Time `gorm:"not null;index"`
	RevokedAt  *time.Time
}
