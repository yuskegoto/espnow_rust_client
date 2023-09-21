# About
- This is ESP-NOW receiving project in Rust, intented for short messaging in WiFi-Range.

## Related projects
[ESP-NOW OSC Station]()
[ESP-NOW Arduino Client]()

## Hardware
I set up this project for ESP32. Button and Serial LED control is intented for [M5Atom](http://docs.m5stack.com/en/core/atom_lite). It is also possible to target the project to ESP32C3.

# Implemented Features
- Send and receive ESP-NOW message from my other project, espnow_osc_station.
- Error message when the ESP-NOW message not reached.

# Setting up environment
For details and newest info please refer [The Rust on ESP Book](https://esp-rs.github.io/book/installation/index.html)
* Install Rust
`
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
`
* Install esp-rs tool chain
`
cargo install espup
espup install
cargo install cargo-generate cargo-espflash espflash cargo-espmonitor espmonitor ldproxy
`

## Configs
- ESP-NOW peer channel can be set via environment variable.
```PowerShell
$env:ESPNOW_CHANNEL = '0'
```
```Bash
export ESPNOW_CHANNEL=0
```
- Device and station MAC addresses can be set in espnow.rs

## Build Commands
```PowerShell
~/export-esp.ps1
```
```Bash
source ~/export-esp.sh
```

```
cargo run
espflash board-info
espmonitor <OCM_PORT_NO>
```

## Build / Run in offline mode
cargo build --offline

# Protocol
## ESP-NOW Packet structure
|Header|Device No|Packet|
|0x72|0x01|0x0A|

## To add message
- Add Msg enum in osc.rs

## Crate
- Using [bbqueue](https://docs.rs/bbqueue/latest/bbqueue/) for between threads communiations.

## References
- [Rust-ESP32-STD-demo](https://github.com/ivmarkov/rust-esp32-std-demo/blob/main/src/main.rs)
- [ESP-NOW Rust sample](https://github.com/esp-rs/esp-wifi/blob/main/examples-esp32/examples/esp_now.rs)
- Tai Hideaki san's [rust-esp32-osc-led](https://github.com/hideakitai/rust-esp32-osc-led.git)
