use anyhow::*;
use std::result::Result::Ok;
use log::*;

use esp_idf_sys::{self as _}; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported

use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use embedded_svc::wifi::{AuthMethod, Configuration, AccessPointConfiguration};

use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::gpio::*;

mod espnow;
use espnow::Espnow;

use bbqueue::BBBuffer;
use bbqueue::framed::FrameProducer;

use smart_leds::hsv::{hsv2rgb, Hsv, RGB};
use smart_leds::SmartLedsWrite;
use ws2812_esp32_rmt_driver::{LedPixelEsp32Rmt, RGB8};
use ws2812_esp32_rmt_driver::driver::color::LedPixelColorGrb24;

pub const MSG_BUF_DOWNSTREAM: usize = 32;
static QUEUE_DOWNSTREAM: BBBuffer<MSG_BUF_DOWNSTREAM>= BBBuffer::new();
static mut PRODUCER_DOWNSTREAM: Option<FrameProducer<MSG_BUF_DOWNSTREAM>> = None;

#[allow(dead_code)]
const ESPNOW_CHANNEL_STR: &str = env!("ESPNOW_CHANNEL");

fn main()-> Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    unsafe{
        esp_idf_sys::nvs_flash_init();
    }
    let nvs = EspDefaultNvsPartition::take().unwrap();
    let sysloop = EspSystemEventLoop::take().unwrap();

    // Pin Config
    let peripherals = Peripherals::take().unwrap();

    // Init serial LED pin
    const LED_PIN: u32 = 27;
    let mut ws2812 = LedPixelEsp32Rmt::
    <RGB8, LedPixelColorGrb24>::new(0, LED_PIN).unwrap();
    let button = PinDriver::input(peripherals.pins.gpio39)?;

    // Wifi / ESPNow setting
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs)).unwrap(),
        sysloop.clone(),
    ).unwrap();

    let wifi_configuration: Configuration = Configuration::AccessPoint(AccessPointConfiguration{
        ssid: "espnow".into(),
        ssid_hidden: true,
        channel: 0,
        auth_method: AuthMethod::None,
        ..Default::default()
    }) ;
    wifi.set_configuration(&wifi_configuration)?;
    wifi.start()?;
    info!("Is Wifi started? {:?}", wifi.is_started());

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    let mac = wifi.wifi().ap_netif().get_mac()?;
    info!("mac address: {:X?}", mac);

    info!("ESPNOW Bridge started");

    let peer_channel = ESPNOW_CHANNEL_STR.parse::<u8>().unwrap();

    let (downstream_msg_producer, downstream_msg_consumer) = QUEUE_DOWNSTREAM.try_split_framed().unwrap();
    unsafe {PRODUCER_DOWNSTREAM = Some(downstream_msg_producer);}

    let mut sent = false;

    // Create thread to handle ESPNow messages
    let espnow_join_handle = std::thread::Builder::new()
        .stack_size(4096)
        .spawn(move || {
            let mut espnow = Espnow::new(downstream_msg_consumer);
            espnow.config(peer_channel, &mac);

            espnow.send_boot_msg().unwrap();

            loop {
                if let Err(e) = espnow.run() {
                    error!("Failed to send espnow messages: {e}");
                }

                if button.is_low() {
                    if !sent {
                        info!("push");
                        espnow.send_status().unwrap();
                        sent = true;
                    }

                    let pixels = std::iter::repeat( hsv2rgb(Hsv {
                        hue: 0,
                        sat: 255,
                        val: 128,
                    }))
                    .take(1);
                    ws2812.write(pixels).unwrap();
                }
                else if button.is_high() {
                    if sent {
                        info!("release");
                        sent = false;

                    }

                    let pixels = std::iter::repeat(RGB { r: 0, g: 0, b: 0})
                    .take(1);
                    ws2812.write(pixels).unwrap();
                }

                espnow.idle();
            }
        })?;

    espnow_join_handle.join().unwrap();

    info!("Finish app");
    Ok(())
}
