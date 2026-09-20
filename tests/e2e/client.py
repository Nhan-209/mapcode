"""
tests/e2e/client.py — Stdio JSON-RPC 2.0 MCP Client for MapCode E2E Testing.
"""

import json
import os
import subprocess
import sys
import time
from typing import Any, Dict, List, Optional, Tuple, Union


class McpClientError(Exception):
    """Exception raised when an MCP JSON-RPC call fails or returns an error."""
    def __init__(self, message: str, code: Optional[int] = None, data: Any = None):
        super().__init__(message)
        self.code = code
        self.data = data


class McpClient:
    """
    Subprocess-driven JSON-RPC 2.0 client for MapCode over stdio.
    Works identically against compiled mapcode.exe or pure-Python mock server.
    """

    def __init__(
        self,
        command: Union[str, List[str]],
        cwd: Optional[str] = None,
        timeout: float = 10.0,
        env: Optional[Dict[str, str]] = None,
    ):
        self.command = command
        self.cwd = cwd or os.getcwd()
        self.timeout = timeout
        self.env = env or os.environ.copy()
        self.process: Optional[subprocess.Popen] = None
        self._req_id = 0

    def start(self) -> "McpClient":
        """Start the MCP server subprocess."""
        cmd = self.command if isinstance(self.command, list) else [self.command]
        self.process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=self.cwd,
            env=self.env,
            text=True,
            bufsize=1,
        )
        return self

    def _next_id(self) -> int:
        self._req_id += 1
        return self._req_id

    def send_raw(self, message_obj: Dict[str, Any]) -> None:
        """Serialize and send a JSON-RPC message line over stdin."""
        if not self.process or not self.process.stdin:
            raise McpClientError("Client is not started or stdin is closed")
        line = json.dumps(message_obj) + "\n"
        try:
            self.process.stdin.write(line)
            self.process.stdin.flush()
        except (BrokenPipeError, OSError) as e:
            raise McpClientError(f"Failed to write to server stdin: {e}")

    def read_response(self, expected_id: Optional[Any] = None) -> Dict[str, Any]:
        """
        Read newline-delimited JSON-RPC responses from stdout until a response
        matching expected_id is received.
        """
        if not self.process or not self.process.stdout:
            raise McpClientError("Client is not started or stdout is closed")

        start_time = time.time()
        while True:
            if time.time() - start_time > self.timeout:
                raise TimeoutError(f"Timed out waiting for response to id {expected_id}")

            line = self.process.stdout.readline()
            if not line:
                # Subprocess EOF
                stderr_output = ""
                if self.process.stderr:
                    stderr_output = self.process.stderr.read()
                raise McpClientError(f"Server closed connection unexpectedly. Stderr: {stderr_output}")

            line = line.strip()
            if not line:
                continue

            try:
                msg = json.loads(line)
            except json.JSONDecodeError as e:
                continue  # Skip non-JSON output (e.g. logging)

            # Check if this matches expected_id or is an un-id'd response
            if expected_id is None or msg.get("id") == expected_id:
                return msg

    def send_request(self, method: str, params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Send a JSON-RPC request and wait for matching response."""
        req_id = self._next_id()
        payload = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
        }
        if params is not None:
            payload["params"] = params

        self.send_raw(payload)
        resp = self.read_response(expected_id=req_id)

        if "error" in resp and resp["error"] is not None:
            err = resp["error"]
            raise McpClientError(
                err.get("message", "Unknown error"),
                code=err.get("code"),
                data=err.get("data"),
            )

        return resp.get("result", {})

    def send_notification(self, method: str, params: Optional[Dict[str, Any]] = None) -> None:
        """Send a JSON-RPC notification (no id, no response expected)."""
        payload = {
            "jsonrpc": "2.0",
            "method": method,
        }
        if params is not None:
            payload["params"] = params
        self.send_raw(payload)

    def initialize(self, root_path: str) -> Dict[str, Any]:
        """Execute MCP initialize handshake."""
        result = self.send_request("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "mapcode-e2e-test-client", "version": "1.0.0"},
            "rootPath": os.path.abspath(root_path),
        })
        self.send_notification("notifications/initialized")
        return result

    def list_tools(self) -> List[Dict[str, Any]]:
        """Query tools/list to get all registered MCP tools."""
        result = self.send_request("tools/list")
        return result.get("tools", [])

    def call_tool(self, name: str, arguments: Optional[Dict[str, Any]] = None) -> Any:
        """
        Call a specific MCP tool and automatically unpack result.
        Returns parsed JSON object if result['content'][0]['text'] is JSON.
        """
        result = self.send_request("tools/call", {
            "name": name,
            "arguments": arguments or {},
        })

        content = result.get("content", [])
        if content and isinstance(content, list) and len(content) > 0:
            first_item = content[0]
            if isinstance(first_item, dict) and "text" in first_item:
                text = first_item["text"]
                try:
                    return json.loads(text)
                except (json.JSONDecodeError, TypeError):
                    return text

        return result

    def close(self) -> None:
        """Gracefully close stdin and terminate server process."""
        if self.process:
            try:
                if self.process.stdin and not self.process.stdin.closed:
                    self.process.stdin.close()
                if self.process.stdout and not self.process.stdout.closed:
                    self.process.stdout.close()
                if self.process.stderr and not self.process.stderr.closed:
                    self.process.stderr.close()
                self.process.terminate()
                self.process.wait(timeout=2.0)
            except Exception:
                try:
                    self.process.kill()
                except Exception:
                    pass
            finally:
                self.process = None

    def __enter__(self) -> "McpClient":
        return self.start()

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()
