# eink — weekly weather on a Waveshare e-paper display

Renders a 7-day weather forecast to a **Waveshare 7.5" V2 (800×480) black/white**
e-paper panel on a **Raspberry Pi Zero W** — by drawing images directly, with no
Chromium / headless browser involved.

Weather comes from [Open-Meteo](https://open-meteo.com/) (no API key).

![preview](forecast.png)

## How it works

The pipeline is `config → fetch → render → output`:

- **`config`** — TOML file (location, units, display, refresh).
- **`weather`** — Open-Meteo geocoding + 7-day forecast over HTTPS (`ureq`).
- **`render`** — draws an 800×480 1-bit framebuffer with `embedded-graphics`;
  text via `u8g2-fonts`, weather icons drawn as vector primitives.
- **`output`** — the rendered framebuffer goes to **either** a PNG (host preview)
  **or** the e-paper panel (`epd-waveshare` + `rppal` over SPI).

`render::draw` is generic over the draw target, so the *same* layout code feeds
the PNG and the panel. That also means you can iterate on the layout entirely on
your laptop — no Pi round-trip — and it leaves room to add a daemon + web config
UI later that reuses the exact same core.

## Build & preview on your PC

No hardware needed. The default (`preview`) feature renders to a PNG:

```sh
cargo run --bin render-once -- --config config.toml --output forecast.png
```

Run the tests:

```sh
cargo test
```

## Configuration

See [`config.toml`](config.toml). Units map directly to Open-Meteo:

```toml
[location]
city = "Zagreb"
country = "HR"            # optional, disambiguates the geocoding hit

[units]
temperature = "celsius"   # celsius | fahrenheit
wind = "kmh"             # kmh | ms | mph | kn
precipitation = "mm"      # mm | inch

[display]
model = "epd7in5_v2"
rotation = 0              # 0 | 90 | 180 | 270

[display.pins]            # standard Waveshare HAT wiring (BCM numbers)
reset = 17
dc = 25
busy = 24

[refresh]
interval_minutes = 60
```

## Wiring (Waveshare 7.5" V2 HAT)

Uses SPI0 with hardware chip-select on CE0, plus three GPIOs:

| Panel | Pi (BCM) |
|-------|----------|
| DIN   | MOSI (GPIO10) |
| CLK   | SCLK (GPIO11) |
| CS    | CE0 (GPIO8)  |
| DC    | GPIO25 |
| RST   | GPIO17 |
| BUSY  | GPIO24 |

On the Pi, enable SPI once: `sudo raspi-config` → *Interface Options* → *SPI* →
enable, then reboot. The `pi` user must be in the `spi` and `gpio` groups
(default on Raspberry Pi OS).

## Cross-compile & deploy to the Pi Zero W

The Pi Zero W is ARMv6 hardfloat → target `arm-unknown-linux-gnueabihf`. The
most reliable cross toolchain is [`cross`](https://github.com/cross-rs/cross)
(Docker-based), which ships the C toolchain needed to build the TLS backend:

```sh
cargo install cross
PI_HOST=pi@raspberrypi.local ./deploy/deploy.sh
```

`deploy.sh` cross-builds the `device` binary, copies it plus `config.toml`
(without clobbering an existing one), installs the systemd units, and enables
the hourly timer. Trigger an immediate render with:

```sh
ssh pi@raspberrypi.local sudo systemctl start eink.service
ssh pi@raspberrypi.local journalctl -u eink.service -n 30
```

Building directly on the Pi Zero W also works but is very slow (single-core
ARMv6): `cargo build --release --no-default-features --features device`.

## Roadmap

- [x] One-shot render → PNG preview (host)
- [x] One-shot render → e-paper panel (device) via systemd timer
- [ ] Long-running daemon with an `axum` web UI to edit config and preview the
      screen live — reuses `config` / `weather` / `render` unchanged.
