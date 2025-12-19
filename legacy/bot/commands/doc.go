// Package commands provides shared bot command handlers for all platforms.
// Commands self-register via init() functions when this package is imported.
package commands

// Import this package to trigger command registration via init() functions.
// Example:
//   import _ "pharmabroker/bot/commands"
//
// Then use core.BuildCommands(deps) to create command instances.
