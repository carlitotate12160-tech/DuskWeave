package main

import (
	"flag"
	"fmt"
	"os"
	"os/signal"
	"syscall"
)

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(args []string) error {
	fs := flag.NewFlagSet("httpslab", flag.ContinueOnError)
	listen := fs.String("listen", "127.0.0.1:0", "pinned TCP4 listen address")
	scenario := fs.String("scenario", "200", "closed HTTPS LAB responder scenario")
	counters := fs.String("counters", "", "optional safe JSON counter file path")
	if err := fs.Parse(args); err != nil {
		return err
	}
	svc, err := Start(*listen, *scenario, *counters)
	if err != nil {
		return err
	}
	sigCh := make(chan os.Signal, 2)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
	defer signal.Stop(sigCh)

	done := make(chan struct{})
	go func() {
		svc.Serve()
		close(done)
	}()
	fmt.Printf("listening=%s scenario=%s\n", svc.Addr(), *scenario)
	<-sigCh
	err = svc.Close()
	<-done
	return err
}
