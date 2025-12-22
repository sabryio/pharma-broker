package cronjob

import (
	"pharmabroker/pkg/metrics"
)

// PrometheusMetricsAdapter adapts the pkg/metrics package to cronjob.MetricsCollector.
type PrometheusMetricsAdapter struct{}

// NewPrometheusMetricsAdapter creates a new Prometheus metrics adapter.
func NewPrometheusMetricsAdapter() *PrometheusMetricsAdapter {
	return &PrometheusMetricsAdapter{}
}

func (a *PrometheusMetricsAdapter) Increment(name string, labels map[string]string) {
	job := labels["job"]
	switch name {
	case "cronjob_success_total":
		metrics.CronJobSuccess.WithLabelValues(job).Inc()
	case "cronjob_failed_total":
		metrics.CronJobFailed.WithLabelValues(job).Inc()
	}
}

func (a *PrometheusMetricsAdapter) ObserveDuration(name string, labels map[string]string, seconds float64) {
	job := labels["job"]
	if name == "cronjob_duration_seconds" {
		metrics.CronJobDuration.WithLabelValues(job).Observe(seconds)
	}
}
