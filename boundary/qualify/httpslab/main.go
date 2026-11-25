package main

import (
	"errors"
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
	dnsListen := fs.String("dns-listen", "", "optional pinned UDP4 DNS listen address")
	dnsTTL := fs.Uint("dns-ttl", 30, "synthetic DNS A TTL in seconds")
	dnsRefreshTTL := fs.Uint("dns-refresh-ttl", 0, "optional DNS A TTL for subsequent answers")
	scenario := fs.String("scenario", "200", "closed HTTPS LAB responder scenario")
	counters := fs.String("counters", "", "optional safe JSON counter file path")
	runID := fs.String("run-id", "", "optional safe run identifier for counter binding")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if *dnsTTL > 300 || *dnsRefreshTTL > 300 {
		return errors.New("invalid_dns_ttl")
	}
	svc, err := StartWithOptions(Options{
		ListenAddr:    *listen,
		DNSListenAddr: *dnsListen,
		Scenario:      *scenario,
		CounterPath:   *counters,
		RunID:         *runID,
		DNSTTL:        uint32(*dnsTTL),
		DNSRefreshTTL: uint32(*dnsRefreshTTL),
	})
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
	fmt.Printf("listening=%s dns=%s scenario=%s\n", svc.Addr(), svc.DNSAddr(), *scenario)
	<-sigCh
	err = svc.Close()
	<-done
	return err
}
