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

## Raspberry Pi prerequisites

Before building or running PiViewWall on a Raspberry Pi, make sure the following are installed and working.

### Required tools

- Rust toolchain
- GStreamer 1.x
- required GStreamer plugins for RTSP and video display
- a working Wayland session
- `WAYLAND_DISPLAY` set to the active compositor socket, usually `wayland-0`

### Install on Raspberry Pi OS

```bash
sudo apt update
sudo apt install -y rustc cargo gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly libgstreamer-plugins-base1.0-dev libgstreamer1.0-dev
```

If you are using a Wayland desktop session, verify the display socket:

```bash
echo $WAYLAND_DISPLAY
ls -l /tmp/wayland-* 2>/dev/null
```

A typical working value is:

```bash
WAYLAND_DISPLAY=wayland-0
```

### RTSPS and certificate behavior

Some RTSP cameras require the client to use TCP and to ignore TLS certificate validation when the stream endpoint is not presenting a perfect certificate chain. The working Pi-safe GStreamer pattern is:

```bash
gst-launch-1.0 rtspsrc location="YOUR_CAMERA_URL" protocols=tcp tls-validation-flags=0 latency=100 ! rtph264depay ! decodebin ! videoconvert ! waylandsink display=wayland-0 fullscreen=false render-rectangle="0,0,960,540"
```

This is why the app command generation must include:

- `protocols=tcp`
- `tls-validation-flags=0`
- a sensible latency value such as `latency=100`
- a Wayland display name such as `display=wayland-0`
- a valid `render-rectangle` in the compositor coordinate space, not a legacy `window=x=...` flag

### Required runtime environment

The Pi must have:

- a real display session
- a Wayland compositor running
- the correct `WAYLAND_DISPLAY` value exported
- access to the camera RTSP endpoints
- a valid camera config file

## Build and run instructions

### Build locally on the Pi

```bash
cargo build --release
./target/release/PiViewWall --config /etc/PiViewWall/config.toml
```

### Cross-compile for ARM64

```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

The generated binary will be in:

```bash
target/aarch64-unknown-linux-gnu/release/PiViewWall
```

Copy it to the Pi and run it with the runtime config:

```bash
scp target/aarch64-unknown-linux-gnu/release/PiViewWall pi@<pi-host>:/usr/local/bin/
ssh pi@<pi-host>
/usr/local/bin/PiViewWall --config /etc/PiViewWall/config.toml
```

## Pi test procedure

Use this procedure whenever a camera feed or display issue appears.

1. Verify Rust and GStreamer are installed:

   ```bash
   rustc --version
   gst-launch-1.0 --version
   ```

2. Verify Wayland is active:

   ```bash
   echo $WAYLAND_DISPLAY
   ```

3. Test the raw camera stream manually with the exact working command format:

   ```bash
   gst-launch-1.0 rtspsrc location="rtsp://user:password@10.0.0.10:554/stream1" protocols=tcp tls-validation-flags=0 latency=100 ! rtph264depay ! decodebin ! videoconvert ! waylandsink display=wayland-0 fullscreen=false render-rectangle="0,0,960,540"
   ```

4. If the camera does not appear, confirm the camera endpoint is reachable and the stream is RTSP/H.264.

5. Confirm the compositor is using a Wayland socket, not a direct resolution string. The `display` value is the socket name, not `1920x1080`.

6. If the camera does not appear in the tiled layout, validate the feed rectangle against the display bounds in the layout config and ensure the feed rectangle matches a non-overlapping tile such as `0,0,960,540` or `960,0,1920,540`.

5. Verify the app config file:

   ```bash
   cat /etc/PiViewWall/config.toml
   ```

7. Run the app with the config file:

   ```bash
   /usr/local/bin/PiViewWall --config /etc/PiViewWall/config.toml
   ```

8. Watch logs and confirm the app stays alive without repeated restart loops:

   ```bash
   journalctl -u PiViewWall.service -f
   ```

This procedure should be used as the canonical troubleshooting path for future Pi runtime issues.

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

