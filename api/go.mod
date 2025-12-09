module pharmabroker/api

go 1.25

require (
	github.com/google/uuid v1.6.0
	github.com/rs/zerolog v1.34.0
	golang.org/x/time v0.14.0
	pharmabroker/domain v0.0.0
)

require (
	github.com/mattn/go-colorable v0.1.13 // indirect
	github.com/mattn/go-isatty v0.0.19 // indirect
	golang.org/x/sys v0.12.0 // indirect
)

replace pharmabroker/domain => ../domain
