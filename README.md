# 📡 IronWire

A minimal, secure, and extensible real-time messaging server written in **Rust**, designed as the foundation for a full-featured messenger with voice/video messages, file sharing, and WebRTC calls.

Built with performance, correctness, and modularity in mind — leveraging `axum`, `tokio`, and modern async Rust.

> **Status**: ✅ MVP core complete — authentication, text messaging, and file uploads working.

---

## ✨ Features (Current)

- **WebSocket-based real-time communication**
- **Authentication** via token (JWT-ready)
- **Peer-to-peer text messaging** (online users only)
- **File upload endpoint** (`POST /upload`) with unique URLs
- **Modular architecture** (easy to extend)
- **No external database required** for MVP (state kept in memory)

> Video/audio "circle" messages and WebRTC calls are **not yet implemented** (planned).

---

## 🚀 Quick Start

### Prerequisites

- Rust (1.70+)
- `cargo`
- (Optional) [`websocat`](https://github.com/vi/websocat) for CLI testing

### Run the Server

```bash
git clone https://github.com/zornfeuer/ironwire
cd ironwire
cargo run
```

The server will start on `http://0.0.0.0:8080`.

### Test Authentication & Messaging

1. Open two terminals.
2. In each, connect via WebSocket:

```bash
websocat ws://localhost:8080/ws
```

3. In both, authenticate (use different tokens):

```json
{"type":"auth","payload":{"token":"alice"}}
{"type":"auth","payload":{"token":"bob"}}
```

You should receive:

```json
{"type":"auth_ok"}
```

4. From `alice`, send a message to `bob`:

```json
{"type":"text","payload":{"to":"bob","text":"Hello from Alice!"}}
```

5. `bob` will receive:

```json
{"type":"text","payload":{"from":"alice","text":"Hello from Alice!"}}
```

### Upload a File

Send POST request to `http://localhost:8080/upload` for example with `curl`:
```bash
curl -X POST --data-binary @yourfile.mp4 http://localhost:8080/upload
```

Response:

```json
{"url":"/media/abcd1234.bin"}
```

Access it at: `http://localhost:8080/media/abcd1234.bin`

---

## 📂 Project Structure

```
src/
├── main.rs              # Entry point
├── messages.rs          # Message types (ClientMessage, AppMessage)
├── state.rs             # Shared in-memory state (online users)
├── ws.rs                # Just forwarding ws submodules
├── http.rs              # Just forwarding http submodules
└── ws/                  # WebSocket session logic
    ├── session.rs       # Per-connection state & message handling
    └── handler.rs       # WebSocket upgrade handler
└── http/                # HTTP handlers (upload, fallback)
    ├── upload.rs
    └── fallback.rs
```

---

## 🗺️ Roadmap

| Feature                     | Status       |
|----------------------------|--------------|
| Text messaging             | ✅ Done      |
| File uploads               | ✅ Done      |
| Audio/video "circle" msgs  | ⏳ Planned   |
| End-to-end encryption      | ⏳ Planned   |
| WebRTC voice/video calls   | ⏳ Planned   |
| Message history (SQLite)   | ⏳ Planned   |
| Offline message queue      | ⏳ Planned   |
| Group chats                | ⏳ Planned   |

> The protocol is **custom** (not XMPP or Matrix), allowing full control over features and performance.

---

## 🔒 Security Notes

- All connections should be served over **TLS** in production (add `rustls` support).
- Authentication currently treats the token as the user ID (for MVP).  
  → Will be replaced with **JWT validation**.
- File uploads are stored on disk with random UUIDs (no execution allowed).
- Input validation and rate limiting will be added before production use.

---

## 🛠️ Built With

- [**axum**](https://github.com/tokio-rs/axum) – Web framework
- [**tokio-tungstenite**](https://github.com/snapview/tokio-tungstenite) – WebSocket support
- [**serde**](https://serde.rs) – Serialization
- [**dashmap**](https://github.com/xacrimon/dashmap) – Concurrent in-memory state
- [**tracing**](https://docs.rs/tracing) – Structured logging

---

## 📜 License

MIT — see [LICENSE](LICENSE) for details.

---

> 💡 **Contributions, suggestions, and security feedback are welcome!**  
> This project is designed to be **minimal, auditable, and privacy-respecting** from the ground up.
