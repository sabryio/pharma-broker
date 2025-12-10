module pharmabroker/bot

go 1.25

replace pharmabroker/domain => ../domain

require (
	github.com/go-telegram/bot v1.17.0
	github.com/rs/zerolog v1.34.0
	pharmabroker/domain v0.0.0
)

require (
	github.com/mattn/go-colorable v0.1.14 // indirect
	github.com/mattn/go-isatty v0.0.20 // indirect
	golang.org/x/sys v0.39.0 // indirect
)
