package cronjob

import (
	"github.com/rs/zerolog"
)

// ZerologAdapter adapts zerolog.Logger to the cronjob.Logger interface.
type ZerologAdapter struct {
	log zerolog.Logger
}

// NewZerologAdapter creates a new zerolog adapter.
func NewZerologAdapter(log zerolog.Logger) *ZerologAdapter {
	return &ZerologAdapter{log: log.With().Str("component", "cronjob").Logger()}
}

func (z *ZerologAdapter) Info(msg string, keyvals ...any) {
	event := z.log.Info()
	z.addKeyvals(event, keyvals)
	event.Msg(msg)
}

func (z *ZerologAdapter) Error(msg string, err error, keyvals ...any) {
	event := z.log.Error().Err(err)
	z.addKeyvals(event, keyvals)
	event.Msg(msg)
}

func (z *ZerologAdapter) Debug(msg string, keyvals ...any) {
	event := z.log.Debug()
	z.addKeyvals(event, keyvals)
	event.Msg(msg)
}

func (z *ZerologAdapter) addKeyvals(event *zerolog.Event, keyvals []any) {
	for i := 0; i+1 < len(keyvals); i += 2 {
		key, ok := keyvals[i].(string)
		if !ok {
			continue
		}
		switch v := keyvals[i+1].(type) {
		case string:
			event.Str(key, v)
		case int:
			event.Int(key, v)
		case int64:
			event.Int64(key, v)
		case float64:
			event.Float64(key, v)
		case bool:
			event.Bool(key, v)
		default:
			event.Interface(key, v)
		}
	}
}
