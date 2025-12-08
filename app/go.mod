module pharmabroker/app

go 1.25

require (
	github.com/rs/zerolog v1.34.0
	pharmabroker/ai v0.0.0
	pharmabroker/api v0.0.0
	pharmabroker/domain v0.0.0
	pharmabroker/storage v0.0.0
)

require (
	github.com/dustin/go-humanize v1.0.1 // indirect
	github.com/glebarez/go-sqlite v1.21.2 // indirect
	github.com/glebarez/sqlite v1.11.0 // indirect
	github.com/google/uuid v1.3.0 // indirect
	github.com/jinzhu/inflection v1.0.0 // indirect
	github.com/jinzhu/now v1.1.5 // indirect
	github.com/mattn/go-colorable v0.1.13 // indirect
	github.com/mattn/go-isatty v0.0.19 // indirect
	github.com/remyoudompheng/bigfft v0.0.0-20230129092748-24d4a6f8daec // indirect
	golang.org/x/sys v0.12.0 // indirect
	golang.org/x/text v0.20.0 // indirect
	gorm.io/gorm v1.31.1 // indirect
	modernc.org/libc v1.22.5 // indirect
	modernc.org/mathutil v1.5.0 // indirect
	modernc.org/memory v1.5.0 // indirect
	modernc.org/sqlite v1.23.1 // indirect
)

replace (
	pharmabroker/ai => ../ai
	pharmabroker/api => ../api
	pharmabroker/domain => ../domain
	pharmabroker/pkg => ../pkg
	pharmabroker/storage => ../storage
)
