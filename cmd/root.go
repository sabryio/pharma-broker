package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "pharmabroker",
	Short: "PharmaBroker - WhatsApp pharmaceutical matching platform",
	Long: `PharmaBroker monitors WhatsApp pharmaceutical groups,
uses AI to extract offers and requests, and matches them automatically.

Commands:
  serve    Start the API server and message processor
  monitor  Interactive group selection UI`,
}

// Execute runs the root command
func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func init() {
	// Add subcommands
	rootCmd.AddCommand(serveCmd)
	rootCmd.AddCommand(monitorCmd)
	rootCmd.AddCommand(resetDbCmd)
}
