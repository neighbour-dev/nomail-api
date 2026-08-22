package tables

import (
	"time"

	"gorm.io/gorm"
)

type Device struct {
	gorm.Model

	UserID uint `gorm:"not null;index;uniqueIndex:idx_user_device"`
	User   User `gorm:"constraint:OnDelete:CASCADE;"`

	Identifier string `gorm:"column:device_id;not null;size:128;uniqueIndex:idx_user_device"`
	Name       string `gorm:"size:128"`

	Platform      string `gorm:"not null;default:unknown;size:32;index"`
	DeviceType    string `gorm:"not null;default:unknown;size:32;index"`
	Manufacturer  string `gorm:"size:128"`
	HardwareModel string `gorm:"column:model;size:128"`
	OSVersion     string `gorm:"size:64"`
	AppVersion    string `gorm:"size:64"`
	AppBuild      string `gorm:"size:64"`
	Locale        string `gorm:"size:32"`
	TimeZone      string `gorm:"size:64"`

	IPAddress  string     `gorm:"size:64"`
	UserAgent  string     `gorm:"size:255"`
	LastSeenAt *time.Time `gorm:"index"`
}
