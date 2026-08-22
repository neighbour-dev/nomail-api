package repo

import (
	"errors"
	"nomail-api/internal/platform/db/tables"
	"time"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"
)

type SessionRepo struct {
	db *gorm.DB
}

type LoginDevice struct {
	DeviceID     string
	DeviceName   string
	Platform     string
	DeviceType   string
	Manufacturer string
	Model        string
	OSVersion    string
	AppVersion   string
	AppBuild     string
	Locale       string
	TimeZone     string
	IPAddress    string
	UserAgent    string
	LastSeenAt   *time.Time
	LoggedIn     bool
	PushEnabled  bool
	ExpiresAt    *time.Time
	LoggedInAt   *time.Time
	CreatedAt    time.Time
	UpdatedAt    time.Time
}

func NewSessionRepo(db *gorm.DB) *SessionRepo {
	return &SessionRepo{db: db}
}

// CreateForLogin records the device and replaces its single current session.
func (r *SessionRepo) CreateForLogin(device *tables.Device, session *tables.Session, maxActiveDevices int) error {
	return r.db.Transaction(func(tx *gorm.DB) error {
		now := time.Now()
		device.LastSeenAt = &now
		if err := tx.Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "user_id"}, {Name: "device_id"}},
			DoUpdates: clause.Assignments(map[string]any{
				"name":         device.Name,
				"platform":     device.Platform,
				"device_type":  device.DeviceType,
				"manufacturer": device.Manufacturer,
				"model":        device.HardwareModel,
				"os_version":   device.OSVersion,
				"app_version":  device.AppVersion,
				"app_build":    device.AppBuild,
				"locale":       device.Locale,
				"time_zone":    device.TimeZone,
				"ip_address":   device.IPAddress,
				"user_agent":   device.UserAgent,
				"last_seen_at": now,
				"updated_at":   now,
			}),
		}).Create(device).Error; err != nil {
			return err
		}

		if err := tx.Where("user_id = ? AND device_id = ?", device.UserID, device.Identifier).First(device).Error; err != nil {
			return err
		}

		activeCount, err := countOtherActiveSessions(tx, device.UserID, device.ID, now)
		if err != nil {
			return err
		}
		if activeCount >= int64(maxActiveDevices) {
			if err := revokeOldestActiveSession(tx, device.UserID, device.ID, now); err != nil {
				return err
			}
		}

		session.DeviceID = device.ID
		session.LoggedInAt = now
		if err := tx.Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "device_id"}},
			DoUpdates: clause.Assignments(map[string]any{
				"refresh_token_hash": session.RefreshTokenHash,
				"expires_at":         session.ExpiresAt,
				"logged_in_at":       now,
				"revoked_at":         nil,
				"updated_at":         now,
			}),
		}).Create(session).Error; err != nil {
			return err
		}

		return tx.Where("device_id = ?", device.ID).First(session).Error
	})
}

func (r *SessionRepo) GetValidByHash(tokenHash string) (tables.Session, error) {
	var session tables.Session
	err := r.db.Preload("Device.User").Where(
		"refresh_token_hash = ? AND expires_at > ? AND revoked_at IS NULL",
		tokenHash,
		time.Now(),
	).First(&session).Error
	return session, err
}

// Rotate replaces a refresh token only if the supplied token is still current.
func (r *SessionRepo) Rotate(sessionID uint, oldTokenHash, newTokenHash string, expiresAt time.Time, ipAddress, userAgent string) error {
	return r.db.Transaction(func(tx *gorm.DB) error {
		now := time.Now()
		result := tx.Model(&tables.Session{}).Where(
			"id = ? AND refresh_token_hash = ? AND expires_at > ? AND revoked_at IS NULL",
			sessionID,
			oldTokenHash,
			now,
		).Updates(map[string]any{
			"refresh_token_hash": newTokenHash,
			"expires_at":         expiresAt,
		})
		if result.Error != nil {
			return result.Error
		}
		if result.RowsAffected == 0 {
			return gorm.ErrRecordNotFound
		}

		return tx.Model(&tables.Device{}).
			Where("id = ?", tx.Model(&tables.Session{}).Select("device_id").Where("id = ?", sessionID)).
			Updates(map[string]any{"ip_address": ipAddress, "user_agent": userAgent, "last_seen_at": now}).Error
	})
}

func (r *SessionRepo) TouchDevice(sessionID uint, interval time.Duration) error {
	now := time.Now()
	return r.db.Model(&tables.Device{}).
		Where(
			"id = (?) AND (last_seen_at IS NULL OR last_seen_at < ?)",
			r.db.Model(&tables.Session{}).Select("device_id").Where("id = ?", sessionID),
			now.Add(-interval),
		).
		Update("last_seen_at", now).Error
}

func (r *SessionRepo) Revoke(id uint) error {
	now := time.Now()
	return r.db.Model(&tables.Session{}).
		Where("id = ? AND revoked_at IS NULL", id).
		Update("revoked_at", now).Error
}

func (r *SessionRepo) IsActive(id, userID uint) (bool, error) {
	var count int64
	err := r.db.Model(&tables.Session{}).
		Joins("JOIN devices ON devices.id = sessions.device_id AND devices.deleted_at IS NULL").
		Where("sessions.id = ? AND devices.user_id = ? AND sessions.expires_at > ? AND sessions.revoked_at IS NULL", id, userID, time.Now()).
		Count(&count).Error
	return count == 1, err
}

// ListDevices includes every known device and derives its current login state.
func (r *SessionRepo) ListDevices(userID uint) ([]LoginDevice, error) {
	var devices []LoginDevice
	now := time.Now()
	err := r.db.Table("devices").
		Select(`devices.device_id, devices.name AS device_name, devices.platform, devices.device_type,
			devices.manufacturer, devices.model, devices.os_version, devices.app_version, devices.app_build,
			devices.locale, devices.time_zone, devices.ip_address, devices.user_agent, devices.last_seen_at,
			CASE WHEN sessions.id IS NOT NULL AND sessions.revoked_at IS NULL AND sessions.expires_at > ? THEN TRUE ELSE FALSE END AS logged_in,
			CASE WHEN push_endpoints.id IS NOT NULL AND push_endpoints.enabled = TRUE AND push_endpoints.invalidated_at IS NULL THEN TRUE ELSE FALSE END AS push_enabled,
			sessions.expires_at, sessions.logged_in_at, devices.created_at, devices.updated_at`, now).
		Joins("LEFT JOIN sessions ON sessions.device_id = devices.id AND sessions.deleted_at IS NULL").
		Joins("LEFT JOIN push_endpoints ON push_endpoints.device_id = devices.id AND push_endpoints.deleted_at IS NULL").
		Where("devices.user_id = ? AND devices.deleted_at IS NULL", userID).
		Order("logged_in DESC, devices.updated_at DESC").
		Scan(&devices).Error
	return devices, err
}

func (r *SessionRepo) RevokeByDevice(userID uint, identifier string) (bool, error) {
	var device tables.Device
	if err := r.db.Where("user_id = ? AND device_id = ?", userID, identifier).First(&device).Error; err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return false, nil
		}
		return false, err
	}

	now := time.Now()
	err := r.db.Model(&tables.Session{}).
		Where("device_id = ? AND revoked_at IS NULL", device.ID).
		Update("revoked_at", now).Error
	return true, err
}

func (r *SessionRepo) DeleteDevice(userID uint, identifier string) (bool, error) {
	var device tables.Device
	if err := r.db.Where("user_id = ? AND device_id = ?", userID, identifier).First(&device).Error; err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return false, nil
		}
		return false, err
	}

	err := r.db.Transaction(func(tx *gorm.DB) error {
		if err := tx.Unscoped().Where("device_id = ?", device.ID).Delete(&tables.PushEndpoint{}).Error; err != nil {
			return err
		}
		if err := tx.Unscoped().Where("device_id = ?", device.ID).Delete(&tables.Session{}).Error; err != nil {
			return err
		}
		return tx.Unscoped().Delete(&device).Error
	})
	return true, err
}

func countOtherActiveSessions(tx *gorm.DB, userID, excludedDeviceID uint, now time.Time) (int64, error) {
	var count int64
	err := tx.Model(&tables.Session{}).
		Joins("JOIN devices ON devices.id = sessions.device_id AND devices.deleted_at IS NULL").
		Where("devices.user_id = ? AND sessions.device_id <> ? AND sessions.expires_at > ? AND sessions.revoked_at IS NULL", userID, excludedDeviceID, now).
		Count(&count).Error
	return count, err
}

func revokeOldestActiveSession(tx *gorm.DB, userID, excludedDeviceID uint, now time.Time) error {
	var session tables.Session
	err := tx.Joins("JOIN devices ON devices.id = sessions.device_id AND devices.deleted_at IS NULL").
		Where("devices.user_id = ? AND sessions.device_id <> ? AND sessions.expires_at > ? AND sessions.revoked_at IS NULL", userID, excludedDeviceID, now).
		Order("sessions.logged_in_at ASC").
		First(&session).Error
	if err != nil {
		return err
	}
	return tx.Model(&session).Update("revoked_at", now).Error
}
