# PiViewWall

PiViewWall is the active project name for the modern Raspberry Pi camera wall service.

This repository preserves the original legacy Raspberry Pi camera-wall implementation for reference and compatibility, but the active codebase is now the PiViewWall rebuild.

## Project goal

The old project displays multiple RTSP camera feeds on a Raspberry Pi using OMXPlayer and systemd. The legacy implementation is retained under `legacy/` so it can continue to work without modifying the running installation.

The active goal is to rebuild the application in a more maintainable stack, using Rust and a modern media pipeline, while keeping the legacy assets available for historical comparison and fallback use.

This repo is not archived; the legacy directory is intentionally retained as a preserved reference, not as the primary runtime.

## Legacy system summary

The current legacy implementation uses:

- Bash scripts
- systemd services
- OMXPlayer
- DBus for status / control
- RTSP over TCP
- Raspberry Pi hardware video acceleration

Important legacy files:

- `legacy/displaycameras`
- `legacy/displaycameras.conf`
- `legacy/layout.conf.default`
- `legacy/displaycameras.service`
- `legacy/rotatedisplays`
- `legacy/omxplayer`
- `legacy/omxplayer.bin`
- `legacy/omxplayer_dbuscontrol`

The sanitized config example is in:

- `config/layout.conf.example`
- `config/camera-wall.example.toml`

Do not store credentials, camera tokens, or private stream URLs in source-controlled files.

## Modern rebuild direction

The replacement architecture is planned around:

- Rust for the service layer
- GStreamer for video display and stream handling
- TOML/YAML configuration
- one Pi per room
- custom camera layouts
- automatic restart / health checks
- systemd service startup

This is a better fit than continuing the shell + OMX approach.

## Recommended deployment model

### Option A: build on the Pi itself

This is easiest for development and small deployments.

1. Install Rust on the Pi:

   ```bash
   curl https://sh.rustup.rs -sSf | sh
   source $HOME/.cargo/env
   rustc --version
   cargo --version
   ```

2. Clone the repo:

   ```bash
   git clone <repo-url>
   cd PiViewWall
   ```

3. Build:

   ```bash
   cargo build --release
   ```

4. Run the binary:

   ```bash
   ./target/release/PiViewWall
   ```

This is fine for a development Pi or a single-room install where you do not mind compiling on the device.

### Option B: cross-compile for Pi and distribute the binary

This is the better long-term workflow for production because it avoids compiling on each Pi and makes installs easier.

From a development machine, cross-compile for ARM64 or ARMv7 and publish a release artifact.

Example:

```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

Then copy the built binary to the Pi:

```bash
scp target/aarch64-unknown-linux-gnu/release/PiViewWall pi@<pi-host>:/usr/local/bin/
```

This is the recommended path when you want a simpler rollout and do not want to install the Rust toolchain on every Pi.

## Suggested install flow for the Raspberry Pi

1. Add the binary to `/usr/local/bin/` or a dedicated app folder.
2. Put runtime config in `/etc/PiViewWall/config.toml`.
3. Create a `systemd` service.
4. Enable the service:

   ```bash
   sudo systemctl enable --now PiViewWall.service
   ```

5. Check service status:

   ```bash
   sudo systemctl status PiViewWall.service
   ```

6. View logs:

   ```bash
   journalctl -u PiViewWall.service -f
   ```

## Example systemd service

```ini
[Unit]
Description=PiViewWall
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/PiViewWall --config /etc/PiViewWall/config.toml
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Recommended release strategy

For a real deployment, use this pattern:

- compile once on a dev machine
- produce a release binary for the target Pi architecture
- attach the binary to a GitHub release or keep it in a release artifact bucket
- copy it to the Pi using `scp` or an install script
- run a minimal install script to place the binary and config

This is easier than forcing every Pi to compile from source every time.

## Why not rely on legacy OMX directly?

The legacy runtime is tied to the Raspberry Pi multimedia stack and the ARM 32-bit `omxplayer.bin` binary. That is a strong compatibility issue and is exactly why the modern replacement path is valuable.

## Current repo state

This repository currently starts as a Rust application scaffold while preserving the legacy runtime in `legacy/`.

## Future work

Planned phases:

1. Define config schema for cameras + layout
2. Build the stream manager
3. Render tiled camera windows
4. Add health check and restart logic
5. Add a proper systemd install flow
6. Add Pi-specific release packaging

