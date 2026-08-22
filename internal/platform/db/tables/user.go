package tables

import "gorm.io/gorm"

type User struct {
	gorm.Model
	Username     string `gorm:"uniqueIndex;not null;size:64"`
	PasswordHash string `gorm:"not null;size:255"`

	Devices []Device `gorm:"foreignKey:UserID"`
}
