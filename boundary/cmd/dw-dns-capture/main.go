//go:build linux

package main

import (
	"duskweave/boundary/internal/capture"
	"os"
	"strings"
	"syscall"
)

func main() {
	if len(os.Args) != 1 {
		os.Exit(2)
	}
	if !disableDumps() || !isolatedProcess() {
		os.Exit(1)
	}
	if capture.Run(os.Stdin, os.Stdout) != nil {
		os.Exit(1)
	}
}

func isolatedProcess() bool {
	if os.Getuid() != 65532 || os.Geteuid() != 65532 {
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
