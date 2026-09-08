#!/usr/bin/env python3
"""Minimal JSON-RPC client for voxel-mcp-server (stdio MCP)."""

from __future__ import annotations

import json
import os
import subprocess
from typing import Any


class VoxelMcp:
    def __init__(self, binary: str, project: str, size: int = 32) -> None:
        self.proc = subprocess.Popen(
            [binary, "--project", project, "--size", str(size)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        assert self.proc.stdin is not None
        assert self.proc.stdout is not None
        self._id = 0
        self._framing: str | None = None  # "cl" | "ndjson"

    def close(self) -> None:
        if self.proc.stdin:
            self.proc.stdin.close()
        try:
            self.proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.proc.kill()

    def _send(self, msg: dict[str, Any]) -> None:
        raw = json.dumps(msg, separators=(",", ":")) + "\n"
        assert self.proc.stdin is not None
        self.proc.stdin.write(raw)
        self.proc.stdin.flush()

    def _read_message(self) -> dict[str, Any]:
        assert self.proc.stdout is not None
        while True:
            line = self.proc.stdout.readline()
            if not line:
                err = self.proc.stderr.read() if self.proc.stderr else ""
                raise RuntimeError(f"MCP stdout EOF. stderr={err}")
            line = line.strip()
            if not line:
                continue
            return json.loads(line)

    def call(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        self._id += 1
        msg: dict[str, Any] = {"jsonrpc": "2.0", "id": self._id, "method": method}
        if params is not None:
            msg["params"] = params
        self._send(msg)
        while True:
            reply = self._read_message()
            if reply.get("id") == self._id:
                if "error" in reply:
                    raise RuntimeError(f"{method} error: {reply['error']}")
                return reply
            # skip notifications / logs

    def notify(self, method: str, params: dict[str, Any] | None = None) -> None:
        msg: dict[str, Any] = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            msg["params"] = params
        self._send(msg)

    def initialize(self) -> dict[str, Any]:
        result = self.call(
            "initialize",
            {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "pikachu-gen", "version": "0.1"},
            },
        )
        self.notify("notifications/initialized")
        return result

    def tool(self, name: str, **arguments: Any) -> Any:
        reply = self.call("tools/call", {"name": name, "arguments": arguments})
        result = reply.get("result", reply)
        return result


def default_paths() -> tuple[str, str]:
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    binary = os.path.join(root, "target", "release", "voxel-mcp-server")
    project = os.path.join(root, "project.vox")
    return binary, project
