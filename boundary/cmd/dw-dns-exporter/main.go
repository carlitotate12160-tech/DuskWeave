//go:build linux

package main

import (
	"duskweave/boundary/internal/export"
	"io"
	"os"
	"strings"
	"syscall"
)

func main() {
	if len(os.Args) != 1 && !(len(os.Args) == 2 && os.Args[1] == "--holder") {
		os.Exit(2)
	}
	if !disableDumps() || !isolatedProcess() {
		os.Exit(1)
	}
	if len(os.Args) == 2 {
		// Fixed namespace holder; the actual exporter is exec'd with its private fd3.
		// This process never reads a request, raw material, credentials or a path.
		_, _ = io.CopyN(io.Discard, os.Stdin, 1)
		return
	}
	// Descriptor 3 is a launcher-created private raw pipe, never parent control.
	raw := os.NewFile(3, "capture-input")
	if raw == nil {
		os.Exit(1)
	}
	defer raw.Close()
	if export.Run(os.Stdin, raw, os.Stdout) != nil {
		os.Exit(1)
	}
}

func isolatedProcess() bool {
	if os.Getuid() != 65533 || os.Geteuid() != 65533 {
		return false
	}
	status, err := os.ReadFile("/proc/self/status")
	if err != nil {
		return false
	}
	expected := map[string]string{"CapEff:": "0000000000000000", "CapBnd:": "0000000000000000", "NoNewPrivs:": "1", "Seccomp:": "2"}
	for _, line := range strings.Split(string(status), "\n") {
		fields := strings.Fields(line)
		if len(fields) != 2 {
			continue
		}
		if value, ok := expected[fields[0]]; ok && fields[1] == value {
			delete(expected, fields[0])
		}
	}
	return len(expected) == 0
}

func disableDumps() bool {
	if syscall.Setrlimit(syscall.RLIMIT_CORE, &syscall.Rlimit{Cur: 0, Max: 0}) != nil {
		return false
	}
	_, _, err := syscall.RawSyscall(syscall.SYS_PRCTL, 4, 0, 0)
	if err != 0 {
		return false
	}
	value, _, err := syscall.RawSyscall(syscall.SYS_PRCTL, 3, 0, 0)
	return err == 0 && value == 0
}
