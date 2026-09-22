# PiViewWall — Legacy Inventory

## Original components

- `displaycameras` — main camera display/management script
- `displaycameras.conf` — global configuration
- `layout.conf.default` — camera/window layout and RTSP feeds
- `displaycameras.service` — systemd service definition
- `rotatedisplays` — background camera rotation helper
- `omxplayer` — OMXPlayer launcher
- `omxplayer.bin` — ARM 32-bit OMXPlayer binary
- `omxplayer_dbuscontrol` — DBus control wrapper

## Architecture

The original system:

1. Runs `displaycameras` as root.
2. Loads configuration from `/etc/displaycameras/`.
3. Starts one OMXPlayer process per camera.
4. Uses RTSP over TCP.
5. Uses DBus to monitor/control individual OMXPlayer instances.
6. Positions each video using the `--win` coordinates from the layout.
7. Can rotate camera feeds between display windows.
8. Uses a systemd service to start the camera wall at boot.

## Important compatibility note

`omxplayer.bin` is an ARM 32-bit executable and depends on the Raspberry Pi multimedia stack. It should be treated as a legacy runtime artifact rather than a portable x86 application.

## Current legacy version

displaycameras: 0.8.3
