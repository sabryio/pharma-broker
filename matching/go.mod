module pharmabroker/matching

go 1.25

require (
	github.com/robfig/cron/v3 v3.0.1
	pharmabroker/domain v0.0.0
)

replace pharmabroker/domain => ../domain
