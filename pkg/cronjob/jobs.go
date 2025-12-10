package cronjob

import (
	"context"
)

// FuncJob wraps a simple function as a Job.
// Useful for one-off or anonymous jobs.
type FuncJob struct {
	id       string
	schedule string
	fn       func(ctx context.Context) error
}

// NewFuncJob creates a new function-based job.
func NewFuncJob(id, schedule string, fn func(ctx context.Context) error) *FuncJob {
	return &FuncJob{
		id:       id,
		schedule: schedule,
		fn:       fn,
	}
}

func (f *FuncJob) ID() string                    { return f.id }
func (f *FuncJob) Schedule() string              { return f.schedule }
func (f *FuncJob) Run(ctx context.Context) error { return f.fn(ctx) }

// IntervalJob is a convenience for creating interval-based jobs.
// Use @every syntax: @every 1h, @every 30m, @every 5s
type IntervalJob struct {
	FuncJob
}

// NewIntervalJob creates a job that runs at a fixed interval.
// Examples: "1h" for hourly, "30m" for every 30 minutes, "10s" for every 10 seconds.
func NewIntervalJob(id, interval string, fn func(ctx context.Context) error) *IntervalJob {
	return &IntervalJob{
		FuncJob: FuncJob{
			id:       id,
			schedule: "@every " + interval,
			fn:       fn,
		},
	}
}

// DailyJob is a convenience for creating daily jobs at a specific time.
type DailyJob struct {
	FuncJob
}

// NewDailyJob creates a job that runs daily at the specified hour and minute.
// hour: 0-23, minute: 0-59
func NewDailyJob(id string, hour, minute int, fn func(ctx context.Context) error) *DailyJob {
	// Cron format: minute hour * * * (min hour day-of-month month day-of-week)
	schedule := formatCron(intToStr(minute), intToStr(hour), "*", "*", "*")
	return &DailyJob{
		FuncJob: FuncJob{
			id:       id,
			schedule: schedule,
			fn:       fn,
		},
	}
}

// HourlyJob is a convenience for creating hourly jobs.
type HourlyJob struct {
	FuncJob
}

// NewHourlyJob creates a job that runs every hour at the specified minute.
// minute: 0-59
func NewHourlyJob(id string, minute int, fn func(ctx context.Context) error) *HourlyJob {
	schedule := formatCron(intToStr(minute), "*", "*", "*", "*")
	return &HourlyJob{
		FuncJob: FuncJob{
			id:       id,
			schedule: schedule,
			fn:       fn,
		},
	}
}

// formatCron builds a cron expression from components.
func formatCron(minute, hour, dayOfMonth, month, dayOfWeek string) string {
	return minute + " " + hour + " " + dayOfMonth + " " + month + " " + dayOfWeek
}

func intToStr(n int) string {
	if n == 0 {
		return "0"
	}
	if n < 0 {
		return "-" + intToStr(-n)
	}
	result := ""
	for n > 0 {
		result = string(rune('0'+n%10)) + result
		n /= 10
	}
	return result
}
