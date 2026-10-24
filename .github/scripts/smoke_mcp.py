"""Start a built math-mcp, send an MCP initialize and require a proper reply within 20 s.
Usage: python smoke_mcp.py <binary> [version it must report]"""
import json
import subprocess
import sys
import threading

REQUEST = {"jsonrpc": "2.0", "id": 1, "method": "initialize",
           "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "release-smoke", "version": "1"}}}

server = subprocess.Popen([sys.argv[1]], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding="utf-8")
reply = []
reader = threading.Thread(target=lambda: reply.append(server.stdout.readline()), daemon=True)
reader.start()
server.stdin.write(json.dumps(REQUEST) + "\n")
server.stdin.flush()
reader.join(20)
server.kill()
if not reply or not reply[0].strip():
    sys.exit("no reply to initialize within 20 s")
answer = json.loads(reply[0])
info = answer.get("result", {}).get("serverInfo")
if answer.get("id") != 1 or not info:
    sys.exit(f"unexpected reply: {reply[0][:300]}")
if len(sys.argv) > 2 and info.get("version") != sys.argv[2]:
    sys.exit(f"the binary says version {info.get('version')}, the release is {sys.argv[2]}")
print(f"initialize answered by {info.get('name')} {info.get('version')}")
