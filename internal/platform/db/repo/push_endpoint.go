package repo

import (
	"errors"
	"nomail-api/internal/platform/db/tables"
	"time"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"
)

type PushEndpointRepo struct {
	db *gorm.DB
}

func NewPushEndpointRepo(db *gorm.DB) *PushEndpointRepo {
	return &PushEndpointRepo{db: db}
}

func (r *PushEndpointRepo) Upsert(userID uint, deviceIdentifier string, endpoint *tables.PushEndpoint) error {
	return r.db.Transaction(func(tx *gorm.DB) error {
		device, err := getOwnedDevice(tx, userID, deviceIdentifier)
		if err != nil {
			return err
		}

		now := time.Now()
		endpoint.DeviceID = device.ID
		endpoint.RegisteredAt = now
		endpoint.Enabled = true
		if err := tx.Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "device_id"}},
			DoUpdates: clause.Assignments(map[string]any{
				"provider":        endpoint.Provider,
				"environment":     endpoint.Environment,
				"encrypted_token": endpoint.EncryptedToken,
				"token_hash":      endpoint.TokenHash,
				"enabled":         true,
				"registered_at":   now,
				"invalidated_at":  nil,
				"last_error":      "",
				"updated_at":      now,
			}),
		}).Create(endpoint).Error; err != nil {
			return err
		}

		return tx.Where("device_id = ?", device.ID).First(endpoint).Error
	})
}

func (r *PushEndpointRepo) Update(userID uint, deviceIdentifier string, values map[string]any) (tables.PushEndpoint, error) {
	var endpoint tables.PushEndpoint
	err := r.db.Transaction(func(tx *gorm.DB) error {
		device, err := getOwnedDevice(tx, userID, deviceIdentifier)
		if err != nil {
			return err
		}
		if err := tx.Where("device_id = ?", device.ID).First(&endpoint).Error; err != nil {
			return err
		}
		if err := tx.Model(&endpoint).Updates(values).Error; err != nil {
			return err
		}
		return tx.First(&endpoint, endpoint.ID).Error
	})
	return endpoint, err
}

func (r *PushEndpointRepo) Delete(userID uint, deviceIdentifier string) (bool, error) {
	device, err := getOwnedDevice(r.db, userID, deviceIdentifier)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return false, nil
		}
		return false, err
	}

	result := r.db.Unscoped().Where("device_id = ?", device.ID).Delete(&tables.PushEndpoint{})
	return result.RowsAffected > 0, result.Error
}

func getOwnedDevice(db *gorm.DB, userID uint, identifier string) (tables.Device, error) {
	var device tables.Device
	err := db.Where("user_id = ? AND device_id = ?", userID, identifier).First(&device).Error
	return device, err
}
