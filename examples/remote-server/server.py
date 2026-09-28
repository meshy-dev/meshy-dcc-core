"""Minimal standalone MCP service for private or local development."""

from __future__ import annotations

import os
from pathlib import Path
import signal
import time

from meshy_dcc_core import McpHttpConfig
from meshy_dcc_core import create_skill_server

HOST = os.environ.get("MESHY_DCC_HOST", "127.0.0.1")
PORT = int(os.environ.get("MESHY_DCC_PORT", "8765"))
SKILLS_DIR = Path(__file__).parent / "skills"

config = McpHttpConfig(
    port=PORT,
    server_name="studio-service-mcp",
    enable_cors=HOST != "127.0.0.1",
)
config.host = HOST
config.dcc_type = "studio-service"
config.instance_metadata = {"meshy_dcc_instance_type": "standalone"}

server = create_skill_server(
    "studio-service",
    config,
    extra_paths=[str(SKILLS_DIR)],
)
handle = server.start()

print(f"MCP service listening at {handle.mcp_url()}")
print("  identity: studio-service")
print("  lifetime: standalone")
print("Press Ctrl+C to stop.")

running = True


def _stop(_signal_number, _frame):
    global running
    running = False


signal.signal(signal.SIGINT, _stop)
signal.signal(signal.SIGTERM, _stop)

while running:
    time.sleep(1)

handle.shutdown()
print("Service stopped.")
