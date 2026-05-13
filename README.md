# Secure Relay Node Project

This project implements a multi-node end-to-end encrypted (E2EE) real-time mesh messaging layer.

## Architecture Overview

The system consists of two types of nodes:
1. **Intelligent Relay**: A central TCP server that maintains a routing table of connected nodes. It securely routes packets based on their destination ID without being able to decrypt their contents.
2. **Mesh Nodes**: Client nodes that connect to the relay. They generate an ID, broadcast their public key to discover peers, and dynamically negotiate independent end-to-end encrypted sessions with every other node on the network.

## Encryption Flow

- The cryptographic implementation uses `x25519-dalek` for Diffie-Hellman key exchange and `aes-gcm` (AES-256-GCM) for authenticated symmetric encryption.
- When a Node starts, it generates a `StaticSecret` private/public keypair. It broadcasts its public key (`to: 0`) to the Relay, which forwards it to all connected Nodes in a `Handshake` packet.
- Upon receiving a broadcasted public key, other Nodes generate a shared secret for that specific peer, store it in their session list (`HashMap<u32, SessionCrypto>`), and reply with their own public key directly to the sender.
- Every text message is encrypted with a newly generated, randomized 12-byte nonce, which is prepended to the ciphertext before transmission.

## Reliability Mechanisms

- **Structured Packets**: Communication is encapsulated in a `Packet` struct, serialized using `bincode`. It includes `message_id`, `timestamp`, `sender_id`, and `payload_type`.
- **Deduplication**: Nodes track `message_id` to ignore duplicate packets.
- **Acknowledgements & Retries**: Sending a `TextChat` or `Ping` payload enqueues it in a retry pool. Upon receiving an `Ack` payload matching the `message_id`, it is removed. A background task retries unacknowledged messages every few seconds.

## How to Run

### Option 1: Automatic Multi-Node Launch (Recommended)
This will automatically launch the Relay and 3 Nodes (IDs 101, 102, 103) in separate windows using your native terminal emulator (Ghostty, Kitty, GNOME Terminal, CMD, etc).

```bash
cargo run -- all
```

### Option 2: Manual Launch
You can manually start the network across multiple terminal instances.

1. **Start the Relay Node**
   ```bash
   cargo run -- relay
   ```
2. **Start a Mesh Node (Target IP 127.0.0.1, Auto-generate ID)**
   ```bash
   cargo run -- node 127.0.0.1
   ```
3. **Start a Mesh Node (Specific ID)**
   ```bash
   cargo run -- node 127.0.0.1 500
   ```

## Example Chat Session

Once multiple nodes are running, use the interactive terminal commands to chat:
- `/peers`: View a list of all Node IDs you have currently established an encrypted session with.
- `/msg <id> <message>`: Send a direct end-to-end encrypted message to a specific Node ID.
- `/broadcast <message>`: Encrypt and send a message to all connected peers simultaneously.
