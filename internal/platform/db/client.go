package db

import (
	"fmt"
	"log"
	"nomail-api/config"
	"nomail-api/internal/platform/db/tables"
	"time"

	"gorm.io/driver/postgres"
	"gorm.io/gorm"
)

func Init(config config.AppConfig) *gorm.DB {

	dbConfig := ConfigFromAppConfig(config)

	dsn := fmt.Sprintf("host=%s user=%s password=%s dbname=%s port=%d sslmode=disable TimeZone=Asia/Shanghai",
		dbConfig.Host,
		dbConfig.Username,
		dbConfig.Password,
		dbConfig.Database,
		dbConfig.Port,
	)

	db, err := gorm.Open(postgres.Open(dsn), &gorm.Config{})

	if err != nil {
		log.Fatal("连接数据库失败，", err.Error())
	}

	sqlDB, err := db.DB()
	if err != nil {
		log.Fatal(err)
	}

	// 连接池配置
	sqlDB.SetMaxIdleConns(10)           // 最大空闲连接数
	sqlDB.SetMaxOpenConns(100)          // 最大打开连接数
	sqlDB.SetConnMaxLifetime(time.Hour) // 单个连接最大存活时间

	// 自动建表
	err = db.AutoMigrate(&tables.User{}, &tables.Device{}, &tables.Session{}, &tables.WebhookConfig{}, &tables.NotifyMessage{})
	if err != nil {
		log.Fatal("表创建失败", err.Error())
	}

	return db
}
