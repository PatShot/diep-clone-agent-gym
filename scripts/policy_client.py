#!/usr/bin/env python3
"""A policy on the far end of the socket bridge. Standard library only.

Connects to a running simulation, reads one Observation per line, and answers
each with one Action. The policy is the trivial baseline — drive to the nearest
visible shape and shoot it; with nothing in sight, head for the centre — written
in Python so that the bridge is demonstrably policy-agnostic.

    cargo run -p agent --example headless_match --release -- 5 1 bridge
    python3 scripts/policy_client.py 127.0.0.1:7777

The JSON on the wire is exactly schema::Observation in and schema::Action out;
client/src/gen/schema.ts describes both.
"""

import json
import math
import socket
import sys

ARENA_SIDE = 1000.0


def idle():
    return {"control": {"thrust": {"x": 0.0, "y": 0.0}, "aim": 0.0, "fire": False}}


def toward(src, dst):
    dx, dy = dst["x"] - src["x"], dst["y"] - src["y"]
    length = math.hypot(dx, dy)
    if length < 1e-6:
        return {"x": 0.0, "y": 0.0}
    return {"x": dx / length, "y": dy / length}


def decide(obs):
    own = obs["own"]
    if own.get("entity") is None:
        return idle()
    me = own["pos"]
    shapes = [v for v in obs.get("visible", []) if v["style"]["kind"] == "shape"]
    if shapes:
        target = min(
            shapes,
            key=lambda v: (v["position"]["pos"]["x"] - me["x"]) ** 2
            + (v["position"]["pos"]["y"] - me["y"]) ** 2,
        )
        d = toward(me, target["position"]["pos"])
        fire = bool(own["reload_ready"])
    else:
        d = toward(me, {"x": ARENA_SIDE * 0.5, "y": ARENA_SIDE * 0.5})
        fire = False
    return {"control": {"thrust": d, "aim": math.atan2(d["y"], d["x"]), "fire": fire}}


def connect_with_patience(host, port, seconds):
    """Keep trying until the simulation is listening, or give up."""
    import time

    deadline = time.monotonic() + seconds
    while True:
        try:
            return socket.create_connection((host, port), timeout=1.0)
        except OSError:
            if time.monotonic() > deadline:
                raise
            time.sleep(0.2)


def main():
    addr = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1:7777"
    host, port = addr.rsplit(":", 1)
    sock = connect_with_patience(host, int(port), seconds=30.0)
    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    reader = sock.makefile("r", encoding="utf-8")
    decided = 0
    try:
        for line in reader:
            obs = json.loads(line)
            action = decide(obs)
            sock.sendall((json.dumps(action) + "\n").encode("utf-8"))
            decided += 1
    except (BrokenPipeError, ConnectionResetError):
        pass
    finally:
        print(f"decided {decided} times", file=sys.stderr)
        sock.close()


if __name__ == "__main__":
    main()
