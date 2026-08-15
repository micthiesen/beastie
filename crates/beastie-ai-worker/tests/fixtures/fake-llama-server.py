#!/usr/bin/env python3
"""Small loopback-only llama-server stand-in for Rust integration tests."""

import argparse
import json
import os
import socket
import sys
import time


parser = argparse.ArgumentParser(add_help=False)
parser.add_argument("--host", required=True)
parser.add_argument("--port", required=True, type=int)
parser.add_argument("--api-key", required=True)
parser.add_argument("--fake-mode", default="valid")
parser.add_argument("--fake-record")
args, unknown = parser.parse_known_args()


def record(value):
    if args.fake_record:
        with open(args.fake_record, "a", encoding="utf-8") as file:
            file.write(value + "\n")


record("start pid=%d argv=%s" % (os.getpid(), " ".join(sys.argv[1:])))
server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind((args.host, args.port))
server.listen(8)


def read_request(connection):
    data = b""
    while b"\r\n\r\n" not in data:
        piece = connection.recv(4096)
        if not piece:
            return "", {}, b""
        data += piece
    header, body = data.split(b"\r\n\r\n", 1)
    lines = header.decode("ascii").split("\r\n")
    method, path, _ = lines[0].split(" ", 2)
    headers = {}
    for line in lines[1:]:
        name, value = line.split(":", 1)
        headers[name.lower()] = value.strip()
    length = int(headers.get("content-length", "0"))
    while len(body) < length:
        body += connection.recv(length - len(body))
    return method + " " + path, headers, body


def reply(connection, status, body):
    connection.sendall(("HTTP/1.1 %d OK\r\nContent-Length: %d\r\nConnection: close\r\n\r\n" % (status, len(body))).encode("ascii") + body)


while True:
    connection, _ = server.accept()
    with connection:
        request, headers, body = read_request(connection)
        authorized = headers.get("authorization") == "Bearer " + args.api_key
        record("request=%s auth=%s bytes=%d body=%s" % (request, authorized, len(body), body.decode("utf-8", "replace")))
        if not authorized:
            reply(connection, 401, b"{}")
            continue
        if request == "GET /health":
            reply(connection, 200, b"{}")
            continue
        if args.fake_mode == "timeout":
            time.sleep(5)
            continue
        if args.fake_mode == "oversized":
            connection.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n")
            continue
        if args.fake_mode == "malformed":
            content = "not a dialogue reply"
        else:
            content = json.dumps({
                "protocol_version": 1,
                "request_id": 41,
                "say": "berry remains bad.",
                "gesture": "look_player",
                "recalled_memory": 41,
            })
        reply(connection, 200, json.dumps({"choices": [{"message": {"content": content}}]}).encode("utf-8"))
