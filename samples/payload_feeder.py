#!/usr/bin/env python3
"""
Feed a rotating set of JSON payloads into seriallcd over a serial device.
Useful with socat PTYs (one side for seriallcd --device, the other for this script).

Example:
  socat -d -d pty,raw,echo=0 pty,raw,echo=0
  # note the two /dev/pts/* paths printed
  ./samples/payload_feeder.py --device /dev/pts/3 --baud 115200
"""
import argparse
import json
import time
from typing import List, Dict, Any

try:
    import serial  # type: ignore
except ImportError as exc:  # pragma: no cover
    raise SystemExit("pyserial is required: pip install pyserial") from exc


def build_frames() -> List[Dict[str, Any]]:
    """Return a list of payloads that exercise the LCD features."""
    return [
        {
            "line1": "HELLO PI | {0x00}{0x01} | Scroll demo for a long line",
            "line2": "IP 192.168.0.99 | Uptime 12:34:56",
            "scroll_speed_ms": 200,
            "page_timeout_ms": 50000,
        },
        {
            "line1": "CPU LOAD",
            "bar_value": 730,
            "bar_max": 1000,
            "page_timeout_ms": 10000,
        },
        {
            "line1": "ALERT: Temp",
            "line2": "85C HOT!",
            "blink": True,
            "ttl_ms": 8000,
        },
        {
            "line1": "Backlight OFF demo",
            "line2": "It should go dark",
            "backlight": False,
            "page_timeout_ms": 4000,
        },
        {
            "line1": "Clear + Test Pattern",
            "line2": "Ensure wiring is OK",
            "page_timeout_ms": 4000,
        },
    ]


def main() -> None:
    parser = argparse.ArgumentParser(description="Send sample payloads to seriallcd")
    parser.add_argument(
        "--device",
        required=True,
        help="Serial device path (e.g., /dev/ttyAMA0 or a PTY from socat)",
    )
    parser.add_argument("--baud", type=int, default=115200, help="Baud rate")
    parser.add_argument(
        "--delay",
        type=float,
        default=4.0,
        help="Seconds to wait between frames",
    )
    args = parser.parse_args()

    ser = serial.Serial(args.device, args.baud, timeout=1)
    frames = build_frames()
    idx = 0
    try:
        while True:
            frame = frames[idx % len(frames)]
            payload = json.dumps(frame, separators=(",", ":"))
            ser.write(payload.encode("utf-8") + b"\n")
            ser.flush()
            idx += 1
            time.sleep(args.delay)
    except KeyboardInterrupt:  # pragma: no cover
        pass
    finally:
        ser.close()


if __name__ == "__main__":  # pragma: no cover
    main()
