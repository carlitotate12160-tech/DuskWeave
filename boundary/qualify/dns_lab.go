// The owned isolated resolver uses real UDP and emits only fixed safe counters.
package main

import (
	"bytes"
	"duskweave/boundary/internal/dns"
	"encoding/binary"
	"encoding/json"
	"flag"
	"net"
	"os"
	"path/filepath"
	"sync"
	"time"
)

type counters struct {
	mu     sync.Mutex
	path   string
	values map[string]uint64
}

func (c *counters) add(category string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.values[category]++
	encoded, err := json.Marshal(c.values)
	if err != nil {
		os.Exit(1)
	}
	temporary := c.path + ".pending"
	if os.WriteFile(temporary, encoded, 0600) != nil || os.Rename(temporary, c.path) != nil {
		os.Exit(1)
	}
}

func main() {
	path := flag.String("counter-file", "", "safe counter file in qualification report directory")
	probe := flag.Bool("probe", false, "fixed isolated LAB egress-negative probes")
	flag.Parse()
	if *probe {
		if flag.NArg() != 0 || *path != "" {
			os.Exit(2)
		}
		probeIsolated()
		return
	}
	if flag.NArg() != 0 || !filepath.IsAbs(*path) {
		os.Exit(2)
	}
	if _, err := os.Lstat(*path); !os.IsNotExist(err) {
		os.Exit(2)
	}
	c := &counters{path: *path, values: map[string]uint64{"dns_udp": 0, "wrong_udp_port": 0, "dns_tcp": 0, "unrecognized": 0}}
	udp, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(192, 0, 2, 53), Port: 53})
	if err != nil {
		os.Exit(1)
	}
	trap, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(192, 0, 2, 53), Port: 54})
	if err != nil {
		os.Exit(1)
	}
	tcp, err := net.ListenTCP("tcp4", &net.TCPAddr{IP: net.IPv4(192, 0, 2, 53), Port: 53})
	if err != nil {
		os.Exit(1)
	}
	go func() {
		packet := make([]byte, 4097)
		for {
			if _, _, err := trap.ReadFromUDP(packet); err != nil {
				return
			}
			c.add("wrong_udp_port")
		}
	}()
	go func() {
		for {
			conn, err := tcp.AcceptTCP()
			if err != nil {
				return
			}
			c.add("dns_tcp")
			conn.Close()
		}
	}()
	// Publication after all three listeners exist is the qualification readiness signal.
	c.add("ready")
	packet := make([]byte, 4097)
	for {
		n, peer, err := udp.ReadFromUDP(packet)
		if err != nil {
			os.Exit(1)
		}
		c.add("dns_udp")
		response, scenario := answer(packet[:n])
		if scenario == "" {
			c.add("unrecognized")
			continue
		}
		c.add(scenario)
		if response != nil {
			_, _ = udp.WriteToUDP(response, peer)
		}
	}
}

func answer(packet []byte) ([]byte, string) {
	if len(packet) < 12 || len(packet) > 512 {
		return nil, ""
	}
	for _, scenario := range []string{"seed", "ttl-zero", "ttl-one", "nxdomain", "empty", "truncated", "timeout", "malformed", "sentinel", "wrong-id"} {
		query, err := dns.Query(binary.BigEndian.Uint16(packet), scenario+".duskweave.test")
		if err != nil || !bytes.Equal(query, packet) {
			continue
		}
		if scenario == "timeout" {
			return nil, scenario
		}
		result := bytes.Clone(query)
		result[2], result[3] = 0x81, 0x80
		switch scenario {
		case "nxdomain":
			result[3] |= 3
			return result, scenario
		case "empty":
			return result, scenario
		case "truncated":
			result[2] |= 2
			return result, scenario
		case "malformed":
			result[12], result[13] = 0xc0, 12
			return result, scenario
		case "sentinel":
			result[7] = 1
			return append(result, []byte("RAW_DNS_SENTINEL_MUST_NOT_ESCAPE")...), scenario
		case "wrong-id":
			result[1]++
		}
		ttl := byte(30)
		if scenario == "ttl-zero" {
			ttl = 0
		}
		if scenario == "ttl-one" {
			ttl = 1
		}
		result[7] = 1
		return append(result, 0xc0, 12, 0, 1, 0, 1, 0, 0, 0, ttl, 0, 4, 198, 51, 100, 7), scenario
	}
	return nil, ""
}

// This qualifier never accepts destinations or runs in the initial host network namespace.
func probeIsolated() {
	current, err := os.Readlink("/proc/self/ns/net")
	initial, other := os.Readlink("/proc/1/ns/net")
	if err != nil || other != nil || current == initial {
		os.Exit(2)
	}
	results := map[string]bool{}
	for name, address := range map[string]string{
		"resolver_udp": "192.0.2.53:53", "wrong_udp_port": "192.0.2.53:54",
		"wrong_resolver": "192.0.2.54:53", "metadata": "169.254.169.254:53",
	} {
		results[name] = probeUDP("udp4", address)
	}
	results["ipv6"] = probeUDP("udp6", "[2001:db8::53]:53")
	conn, err := net.DialTimeout("tcp4", "192.0.2.53:53", 20*time.Millisecond)
	results["dns_tcp"] = err == nil
	if conn != nil {
		conn.Close()
	}
	encoded, err := json.Marshal(results)
	if err != nil {
		os.Exit(1)
	}
	os.Stdout.Write(encoded)
}
func probeUDP(network, address string) bool {
	conn, err := net.DialTimeout(network, address, 20*time.Millisecond)
	if err != nil {
		return false
	}
	defer conn.Close()
	if conn.SetDeadline(time.Now().Add(20*time.Millisecond)) != nil {
		return false
	}
	if _, err = conn.Write([]byte("LAB_EGRESS_PROBE")); err != nil {
		return false
	}
	var response [1]byte
	_, err = conn.Read(response[:])
	return err == nil
}
