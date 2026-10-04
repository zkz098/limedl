# Files

- [Aria2 JSON-RPC Compatibility Server](aria2-rpc-server.md) - The aria2-compatible HTTP and WebSocket server of limedl — method routing, GID derivation and caching, three auth modes with Argon2 token storage, the configurable bind address with a fail-closed auth gate, CORS wildcard mode, type-driven request parsing and aria2 option translation, the tellStatus/getServers/changeUri/changePosition surface, notification ownership, exit_on_shutdown and the graceful hot-reload port handoff.
- [Headless Server Daemon (limedl-server)](headless-server-daemon.md) - The GUI-free limedl-server binary — CLI and data-dir resolution, the data-directory instance lock, forced-enabled Aria2 RPC with exit_on_shutdown, SIGTERM/SIGINT shutdown, static musl packaging and the end-to-end daemon test.
