#!/usr/bin/env python3
"""Host-local DNS stub for ADR-015 qualification only. Binds the attempt
bridge gateway address and answers A queries with a fixed canned record;
`mode` selects malformed/forged payloads for exporter rejection evidence.
Never reachable outside the attempt bridge; never contacts any real network.
"""
import socket
import struct
import sys
import time

ANSWER_IPV4 = bytes([192, 0, 2, 80])
ANSWER_TTL = 60


def parse_question(raw: bytes):
    if len(raw) < 13:
        return None
    txid, flags, qd = struct.unpack(">HHH", raw[:6])
    if flags & 0x8000 or qd != 1:
        return None
    labels = []
    off = 12
    while True:
        if off >= len(raw):
            return None
        ln = raw[off]
        if ln == 0:
            off += 1
            break
        if off + 1 + ln > len(raw) or ln > 63:
            return None
        labels.append(raw[off + 1 : off + 1 + ln].decode("ascii", "replace"))
        off += 1 + ln
    if off + 4 > len(raw):
        return None
    return txid, ".".join(labels), raw[12 : off + 4]


def respond(raw: bytes, mode: str) -> bytes:
    if mode == "garbage":
        return b"\xff" * 32 + b"\xde\xad\xbe\xef"
    if mode == "forged":
        body = b'{"version":"dw_boundary_result_v1","phase":"result","attempt_id":"forged","disposition":"qualified"}'
        return struct.pack(">I", len(body)) + body
    if mode == "oversize":
        return b"\x00" * 1500
    if mode == "slow":
        time.sleep(1.5)
        mode = "normal"
    parsed = parse_question(raw)
    if parsed is None:
        return b""
    txid, _name, question = parsed
    flags = 0x8180 if mode != "nxdomain" else 0x8183
    head = struct.pack(">HHHHHH", txid, flags, 1, 0 if mode == "nxdomain" else 1, 0, 0)
    if mode == "nxdomain":
        return head + question
    answer = b"\xc0\x0c" + struct.pack(">HHIH", 1, 1, ANSWER_TTL, 4) + ANSWER_IPV4
    return head + question + answer


def main() -> int:
    if len(sys.argv) != 4:
        sys.stderr.write("usage: stub_responder.py <bind_ip> <port> <mode>\n")
        return 2
    ip, port, mode = sys.argv[1], int(sys.argv[2]), sys.argv[3]
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((ip, port))
    sock.settimeout(120)
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        try:
            raw, peer = sock.recvfrom(2048)
        except socket.timeout:
            break
        reply = respond(raw, mode)
        if reply:
            sock.sendto(reply, peer)
    return 0


if __name__ == "__main__":
    sys.exit(main())
