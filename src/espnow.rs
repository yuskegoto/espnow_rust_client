use anyhow::Result;
use std::time::Duration;
use bbqueue::framed::FrameConsumer;
use log::*;
use esp_idf_hal::reset::restart;

use rand::{Rng, rngs::ThreadRng};

extern crate num;
extern crate num_derive;
use num_derive::FromPrimitive;

use esp_idf_sys::{self as _, esp_interface_t_ESP_IF_WIFI_AP}; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use esp_idf_svc::espnow::*;

const ESPNOW_FRAME_INTERVAL_MS: Duration = Duration::from_millis(1);
use crate::{PRODUCER_DOWNSTREAM, MSG_BUF_DOWNSTREAM};

// Adding device's MAC address to NODE_ADDRESSES const
const BROADCAST: [u8;6] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
const STATION_MAC: [u8;6] = [0x48, 0xE7, 0x29, 0x24, 0x81, 0x29];
const DEVICE1_MAC: [u8;6] = [0x50, 0x02, 0x91, 0x9F, 0xCF, 0x9C];
const DEVICE2_MAC: [u8;6] = [0x50, 0x02, 0x91, 0x87, 0x95, 0x81];
const NODE_ADDRESSES: [[u8;6]; 3] = [BROADCAST, DEVICE1_MAC, DEVICE2_MAC];

#[allow(dead_code)]
#[derive(FromPrimitive, Clone, Copy)]
pub enum Msg {
    None = 0,
    Boot = 0x42,            // Boot report
    Mac = 0x4D,             // MAC report
    Status = 0x55,          // statUs

    Reset = 0x62,           // b:reBoot
    MacQuery = 0x6D,        // m:Mac address query
    Run = 0x72,             // r:Run
    StatusQuery = 0x75,     // u:statUs
}

pub struct Espnow{
    receiver: FrameConsumer<'static, MSG_BUF_DOWNSTREAM>,
    espnow: EspNow,
    dev_no: u8,
    rng: ThreadRng,
    mac: [u8;6],
}

impl Espnow{
    pub fn new(receiver: FrameConsumer<'static, MSG_BUF_DOWNSTREAM>) -> Self {
        let espnow = EspNow::take().unwrap();
        let _ = espnow.register_recv_cb(recv_callback).unwrap();
        let _ = espnow.register_send_cb(send_callback).unwrap();
        let dev_no = 0;
        let rng = rand::thread_rng();
        let mac = [0u8;6];

        Self {
            receiver,
            espnow,
            dev_no,
            rng,
            mac,
        }
    }

    /**
     * Adding peer addresses to peer list
    */
    pub fn config(&mut self, peer_channel: u8, mac_addr:&[u8]){
        let peer_info = PeerInfo {
            peer_addr: STATION_MAC,
            lmk: [0u8; 16],
            channel: peer_channel,
            encrypt: false,
            ifidx: esp_interface_t_ESP_IF_WIFI_AP,
            priv_: std::ptr::null_mut(),
        };
        if let Err(e) = self.espnow.add_peer(peer_info){
            error!("ESPNOW add peer error: {e}");
        };

        // Find device no
        self.mac.copy_from_slice(mac_addr);
        for (i, n) in NODE_ADDRESSES.iter().enumerate(){
            if n == &self.mac {
                self.dev_no = i as u8;
                info!("My number is {}", self.dev_no);
                break;
            }
        }
    }

    /**
     * On receiving espnow packet, return echo.
    */
    pub fn run(&mut self) -> Result<Msg> {
        if let Some(frame) = self.receiver.read() {
            let mut data = [0u8; 10];
            data[..frame.len()].copy_from_slice(&frame);
            let target_no = data[1] as usize;
            info!("Received downstream msg to:{target_no}");

            if NODE_ADDRESSES.len() >= target_no && target_no == self.dev_no as usize {
                let msg_type = num::FromPrimitive::from_u8(data[0]);
                let mut msgbuf = [0u8;10];
                msgbuf[1] = self.dev_no;

                let msg_len = match msg_type {
                    Some(Msg::MacQuery) => {
                        msgbuf[0] = Msg::Mac as u8;
                        msgbuf[2..8].copy_from_slice(&self.mac);
                        6
                    }
                    Some(Msg::Reset) => {
                        self.reset_sequence();
                        0
                    }
                    Some(Msg::StatusQuery) => {
                        msgbuf[0] = Msg::Status as u8;
                        msgbuf[2] = self.rng.gen();
                        1
                    }
                    Some(Msg::Run) => {
                        info!("Got run msg!");
                        0
                    }

                    _ => {
                        0
                    }
                // self.send(Msg::Status).unwrap();
                };
                if msg_len > 0 {
                    self.send_slice(&msgbuf[..(msg_len + 2)])?;
                }
            }
            frame.release();
        }
        Ok(Msg::None)
    }

    /**
     * Sleep the thread until next interval
    */
    pub fn idle(&self) {
        std::thread::sleep(ESPNOW_FRAME_INTERVAL_MS);
    }

    /**
     * Send out ESPnow status message.
    */
    pub fn send_status(&mut self) -> Result<()> {
        info!("send status!");

        let mut data = [0u8; 3];
        data[0] = Msg::Status as u8;
        data[1] = self.dev_no;
        let r:u8 = self.rng.gen();
        data[2] = r;

        let _ = self.espnow.send(STATION_MAC, &data);

        Ok(())
    }

    /**
     * Send out ESPnow boot message.
    */
    pub fn send_boot_msg(&mut self) -> Result<()> {
        info!("Boot msg");

        let mut data = [0u8; 2];
        data[0] = Msg::Boot as u8;
        data[1] = self.dev_no;

        let _ = self.espnow.send(STATION_MAC, &data);

        Ok(())
    }

    /**
     * On receiving OSC packet, send out ESPnow.
    */
    pub fn send_slice(&mut self, msgbuf: &[u8]) -> Result<()> {
        info!("send slice msg! {:x?}", msgbuf);

        let _ = self.espnow.send(STATION_MAC, msgbuf);

        Ok(())
    }
    /**
     * Reset the device!
    */
    fn reset_sequence(&self){
        // Wait a little bit until all buffer is cleared etc
        std::thread::sleep(Duration::from_millis(100));
        restart();
    }
}

/**
 * ESPnow message callback
 * Messages are simply forwarded to DOWNSTREAM buffer.
*/
fn recv_callback(_recv_info:&[u8], data:&[u8]){
    info!("recv_info:{:X?}, data:{:X?}", _recv_info, data);
    unsafe {if PRODUCER_DOWNSTREAM.is_some(){
        let producer = PRODUCER_DOWNSTREAM.as_mut().unwrap();
        let sz = data.len();
        if let Ok(mut wg) = producer.grant(sz){
            wg.to_commit(sz);
            wg.copy_from_slice(data);
            wg.commit(sz);
        }
        else{
            error!("ESPNOW:Downstream Buffer Overflow!");
        }

    }}
}

/**
 * ESPnow send callback. When espnow send is completed, determine the SendStatus
 * and give back the error osc message when the espnow messge did not reach to destination.
*/
fn send_callback(mac_addr:&[u8], send_status: SendStatus){
    match send_status {
        SendStatus::SUCCESS => {
            info!("send to {:X?} succesfull", mac_addr);
        }
        SendStatus::FAIL => {
            error!("ESPNOW:sending to {:X?} failed!", mac_addr);
        }
    }
}