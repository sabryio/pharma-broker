module pharmabroker/parsing

go 1.25

require pharmabroker/domain v0.0.0

replace (
	pharmabroker/ai => ../ai
	pharmabroker/domain => ../domain
	pharmabroker/pkg => ../pkg
)
