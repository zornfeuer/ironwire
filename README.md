# 📡 IronWire

A minimal, secure, and extensible real-time messaging server written in **Rust**, designed as the foundation for a full-featured messenger with voice/video messages, file sharing, and WebRTC calls.

Built with performance, correctness, and **end-to-end ready security** — leveraging `axum`, `tokio`, `ed25519-dalek`, and modern async Rust.

> **Status**: ✅ Core protocol secure — challenge-response auth, client IDs derived from public keys, memory-safe concurrency.

---

## ✨ Features (Current)

- **WebSocket-based real-time communication**
- **Cryptographic authentication** via Ed25519 challenge-response (no secrets over wire)
- **Client IDs = hex(public key)** — stable, non-reassignable, E2E-ready
- **Peer-to-peer text messaging** (online users only)
- **File upload endpoint** (`POST /upload`) with safe serving (`Content-Disposition: attachment`)
- **DoS-resistant design**:
  - 10s auth timeout
  - Path traversal protection
- **Modular & zero-DB architecture** (in-memory state, `DashMap`)

> Audio/video, WebRTC, message history, and E2E encryption are **planned** (client-side first).

---

## 🚀 Quick Start

### Prerequisites

- Rust ≥1.70
- `cargo`
- (Optional) [`websocat`](https://github.com/vi/websocat) for CLI testing

### Run the Server

```bash
git clone https://github.com/zornfeuer/ironwire
cd ironwire
cargo run
```

Server starts on `http://0.0.0.0:8080`.

---

### 🔐 Test Ed25519 Authentication (via `websocat`)

> **Note**: You need a tool to generate Ed25519 keypair and sign. For demo, use [this script](#appendix-generate-test-keypair) or `client-cli` (coming soon).

#### 1. Generate test keypair (see Appendix)
```bash
# pubkey: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
# privkey: fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210
```

#### 2. Connect & send public key:
```bash
websocat ws://localhost:8080/ws
```
```json
{"type":"auth","payload":{"token":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}}
```

→ Server replies with challenge:
```json
{"type":"auth_challenge","challenge":"a1b2c3d4..."}
```

#### 3. Sign challenge hex-decoded with private key → send signature (hex):
```json
{"type":"verify","payload":{"attempt":"e3f1a9b8..."}}
```

→ On success:
```json
{"type":"auth_ok"}
```

Client ID becomes: `0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef`

#### 4. Send message:
```json
{"type":"text","payload":{"to":"other_client_id","text":"Hello, E2E-ready world!"}}
```

Recipient gets:
```json
{"type":"text","payload":{"from":"your_client_id","text":"Hello..."}}
```

> No shared secrets. No password DB. Server never sees private keys.

---

### 📤 Upload a File (authorized only after auth)

```bash
curl -X POST --data-binary @test.jpg http://localhost:8080/upload
# → {"url":"/media/abcd1234.bin"}

curl http://localhost:8080/media/abcd1234.bin
# → downloads as attachment (XSS-safe)
```

---

## 📂 Project Structure

```
src/
├── main.rs
├── messages.rs          # ClientMessage (auth/verify/text/file), AppMessage
├── state.rs             # SharedState = DashMap<ClientId, Sender>
├── ws/
│   ├── handler.rs       # WebSocket upgrade
│   ├── session.rs       # Auth state, challenge, multi-device ready
│   └── auth.rs          # Ed25519 challenge/response logic ✅ NEW
└── http/
    ├── upload.rs        # Size-limited, safe
    ├── media.rs         # Traversal-protected, attachment-only ✅ NEW
    └── fallback.rs
```

---

## 🗺️ Roadmap

| Feature                    | Status       | Notes |
|----------------------------|--------------|-------|
| Ed25519 auth               | ✅ Done      | Challenge-response, client IDs = pubkey |
| Text messaging             | ✅ Done      | Online only |
| Secure file serving        | ✅ Done      | `attachment`, no traversal |
| Upload size limit (30 MiB) | ✅ Done      | Via `RequestBodyLimitLayer` |
| Multi-device support       | ⏳ Planned   | One client ID → many sessions |
| Group chats                | ⏳ Planned   | Room-based, client-coordinated |
| Offline message queue      | ⏳ Planned   | With sled/SQLite |
| **E2E encryption**         | ⏳ Planned   | X25519 + ChaCha20-Poly1305 (client-core) |
| Voice/video circle msgs    | ⏳ Planned   | Client-side recording → upload → notify |
| WebRTC signalling          | ⏳ Planned   | SDP over existing WS channel |

> Protocol is **custom**, minimal, and designed for **auditability + privacy**.

---

## 🔒 Security Model

| Layer | Mechanism |
|------|-----------|
| **Auth** | Ed25519 challenge-response (no secrets transmitted) |
| **IDs** | `client_id = hex(pubkey)` — immutable, non-spoofable |
| **Files** | Random UUID names, `Content-Disposition: attachment`, no MIME sniffing |
| **Network** | No TLS in dev — **must be fronted by HTTPS (e.g. Caddy/Nginx)** in prod |
| **DoS** | Auth timeout, upload limit, traversal protection |
| **E2E path** | Server only transports encrypted payloads — keys never leave clients |

> ✅ This setup is suitable for threat models where **server compromise ≠ message compromise**.

---

## 🛠️ Built With

- [**axum**](https://github.com/tokio-rs/axum) – Web server
- [**ed25519-dalek**](https://github.com/dalek-cryptography/ed25519-dalek) – Cryptographic auth ✅
- [**dashmap**](https://github.com/xacrimon/dashmap) – Concurrent in-memory state
- [**serde**](https://serde.rs) – Message serialization
- [**tower-http**](https://github.com/tower-rs/tower-http) – Request body limiting, fallback
- [**tracing**](https://docs.rs/tracing) – Structured logging

---

## 📜 License

MIT — see [LICENSE](LICENSE)

---

> 💡 **Contributions welcome!**  
> Especially:  
> - `client-core` (Rust, E2E logic)  
> - Flutter UI prototype  
> - Fuzzing / audit reports

---

### Appendix: Generate Test Keypair (CLI)

To quickly test auth, use this one-liner with `openssl`:

```bash
# Generate Ed25519 key (OpenSSL 3.0+)
openssl genpkey -algorithm Ed25519 -out priv.pem
openssl pkey -in priv.pem -pubout -outform DER | tail -c 32 | xxd -p -c 32
# → public key (64 hex chars)

# Sign challenge (e.g. "a1b2c3..." as hex string → binary → sign)
echo -n "a1b2c3d4..." | xxd -r -p | openssl pkeyutl -sign -inkey priv.pem -rawin -digest null | xxd -p -c 64
# → signature (128 hex chars)
```

Or use [`client-cli`](https://github.com/zornfeuer/ironwire/tree/main/client-cli) (coming soon).
