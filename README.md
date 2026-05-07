# Secure Relay Node Project

This project extends a basic 3-node system (Sender, Relay, Receiver) by adding an end-to-end encrypted (E2EE) real-time messaging layer.

## Architecture Overview

The system consists of three types of nodes:
1. **Relay**: A headless TCP server that blindly forwards packets between connected clients without being able to decrypt their contents.
2. **Sender & Receiver**: Client nodes that connect to the relay. They perform an initial key exchange and then send/receive encrypted messages using an async terminal chat interface.

## Encryption Flow

- The cryptographic implementation uses `x25519-dalek` for Diffie-Hellman key exchange and `aes-gcm` (AES-256-GCM) for authenticated symmetric encryption.
- When the Sender starts, it generates an ephemeral private/public keypair. It sends its public key to the Relay, which forwards it to the Receiver in a `Handshake` packet.
- The Receiver reads the Sender's public key, generates its own keypair, computes the shared secret, and sends its public key back via a `Handshake` packet.
- Both sides independently derive the exact same `SharedSecret` and use it to instantiate `SessionCrypto` (AES-256-GCM).
- Every text message is encrypted with a newly generated, randomized 12-byte nonce, which is prepended to the ciphertext before transmission.

## Reliability Mechanisms

- **Structured Packets**: Communication is encapsulated in a `Packet` struct, serialized using `bincode`. It includes `message_id`, `timestamp`, `sender_id`, and `payload_type`.
- **Deduplication**: Nodes track `message_id` to ignore duplicate packets.
- **Acknowledgements & Retries**: Sending a `TextChat` or `Ping` payload enqueues it in a retry pool. Upon receiving an `Ack` payload matching the `message_id`, it is removed. A background task retries unacknowledged messages every few seconds.

## How to Run

You will need three separate terminal instances.

1. **Start the Relay Node**
   ```bash
   cargo run -- relay
   ```
2. **Start the Receiver Node**
   ```bash
   cargo run -- receiver
   ```
3. **Start the Sender Node**
   ```bash
   cargo run -- sender
   ```

## Example Chat Session

Once all three nodes are running, the Sender and Receiver will establish a secure connection. You can then type messages in either the Sender or Receiver terminal, and they will be encrypted, forwarded by the Relay, decrypted, and displayed on the other side.
