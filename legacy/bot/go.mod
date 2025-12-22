module pharmabroker/bot

go 1.25

replace pharmabroker/domain => ../domain

require (
	github.com/go-telegram/bot v1.17.0
	github.com/google/uuid v1.6.0
	github.com/mattn/go-runewidth v0.0.19
	github.com/rs/zerolog v1.34.0
	pharmabroker/domain v0.0.0
)

require (
	github.com/clipperhouse/stringish v0.1.1 // indirect
	github.com/clipperhouse/uax29/v2 v2.3.0 // indirect
	github.com/mattn/go-colorable v0.1.14 // indirect
	github.com/mattn/go-isatty v0.0.20 // indirect
	golang.org/x/sys v0.39.0 // indirect
)
