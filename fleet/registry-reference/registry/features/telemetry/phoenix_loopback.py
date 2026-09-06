#!/usr/bin/env python3
"""Run Phoenix's HTTP/UI server without its unusable wildcard gRPC listener.

Phoenix 12.15.1 hardcodes ``[::]:<port>`` for gRPC and does not expose a host override.
Fleet sends OTLP HTTP/protobuf, so disabling only that receiver keeps the actual fleet path
loopback-only while retaining Phoenix's HTTP ingestion and query API.
"""

from phoenix.server.grpc_server import GrpcServer


async def _disabled_grpc_enter(self):
    self._server = None


async def _disabled_grpc_exit(self, *args, **kwargs):
    return None


GrpcServer.__aenter__ = _disabled_grpc_enter
GrpcServer.__aexit__ = _disabled_grpc_exit

from phoenix.server.main import main  # noqa: E402


if __name__ == "__main__":
    main()
