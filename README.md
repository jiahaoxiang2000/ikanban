# iKanban

A Rust-based multi-agent task management system with a core server and multiple client support via HTTP/WebSocket.

## Architecture

```
+------------------+     +------------------+     +------------------+
|   TUI Client     |     |   Web Client     |     |  Other Clients   |
+--------+---------+     +--------+---------+     +--------+---------+
         |                        |                        |
         |    HTTP/WebSocket      |    HTTP/WebSocket      |
         +------------------------+------------------------+
                                  |
                    +-------------+-------------+
                    |      iKanban Core         |
                    |   (HTTP + WebSocket API)  |
                    +-------------+-------------+
                                  |
                    +-------------+-------------+
                    |     SQLite Database       |
                    +---------------------------+
```

## License

MIT
