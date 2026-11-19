//! SmartSecure ESP32 IoT Firmware written in Rust (`esp-idf-svc` + `esp-idf-hal`).
//!
//! Pinout mapping:
//! - PIR HC-SR501  -> GPIO 27 (Digital Input)
//! - MQ-2 Gas      -> GPIO 34 (ADC1_CH6 Analog Input)
//! - Relay Light   -> GPIO 26 (Digital Output)
//! - Buzzer Alarm  -> GPIO 25 (Digital Output)
//! - Door Lock     -> GPIO 33 (Digital Output)

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use embedded_svc::mqtt::client::{EventPayload, QoS};
use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use esp_idf_hal::adc::attenuation::DB_11;
use esp_idf_hal::adc::oneshot::config::AdcChannelConfig;
use esp_idf_hal::adc::oneshot::{AdcChannelDriver, AdcDriver};
use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use serde::{Deserialize, Serialize};

const WIFI_SSID: &str = "CHANGE_ME";
const WIFI_PASSWORD: &str = "CHANGE_ME";
const MQTT_HOST: &str = "192.168.1.10";
const MQTT_PORT: u16 = 1883;
const DEVICE_ID: &str = "ESP32_DEMO_001";

#[derive(Debug, Default, Clone, Copy)]
struct ActuatorState {
    relay: bool,
    buzzer: bool,
    lock: bool,
}

#[derive(Debug, Deserialize)]
struct CommandPayload {
    #[serde(default)]
    command_id: String,
    #[serde(default)]
    action: String,
}

#[derive(Debug, Serialize)]
struct AckPayload<'a> {
    command_id: &'a str,
    device_id: &'a str,
    action: &'a str,
    status: &'a str,
}

#[derive(Debug, Serialize)]
struct StatusPayload<'a> {
    device_id: &'a str,
    status: &'a str,
}

#[derive(Debug, Serialize)]
struct SensorPayload<'a> {
    device_id: &'a str,
    pir: bool,
    mq2: u16,
    relay: bool,
    buzzer: bool,
    lock: bool,
}

fn topic(suffix: &str) -> String {
    format!("security/device/{DEVICE_ID}/{suffix}")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Configure GPIO pins matching hardware schematic
    let pir_pin = PinDriver::input(peripherals.pins.gpio27)?;
    let adc = AdcDriver::new(peripherals.adc1)?;
    let adc_config = AdcChannelConfig {
        attenuation: DB_11,
        ..Default::default()
    };
    let mut mq2_pin = AdcChannelDriver::new(&adc, peripherals.pins.gpio34, &adc_config)?;

    let mut relay_pin = PinDriver::output(peripherals.pins.gpio26)?;
    let mut buzzer_pin = PinDriver::output(peripherals.pins.gpio25)?;
    let mut lock_pin = PinDriver::output(peripherals.pins.gpio33)?;

    relay_pin.set_low()?;
    buzzer_pin.set_low()?;
    lock_pin.set_low()?;

    let shared_relay = Arc::new(Mutex::new(relay_pin));
    let shared_buzzer = Arc::new(Mutex::new(buzzer_pin));
    let shared_lock = Arc::new(Mutex::new(lock_pin));
    let state = Arc::new(Mutex::new(ActuatorState::default()));

    // Connect Wi-Fi
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().unwrap(),
        password: WIFI_PASSWORD.try_into().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;
    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;
    log::info!("Wi-Fi connected successfully");

    // Connect MQTT Broker
    let broker_url = format!("mqtt://{MQTT_HOST}:{MQTT_PORT}");
    let mqtt_config = MqttClientConfiguration {
        client_id: Some(DEVICE_ID),
        keep_alive_interval: Some(Duration::from_secs(30)),
        ..Default::default()
    };

    let (tx_ack, rx_ack) = std::sync::mpsc::channel::<(String, String, &'static str)>();
    let state_cb = Arc::clone(&state);
    let relay_cb = Arc::clone(&shared_relay);
    let buzzer_cb = Arc::clone(&shared_buzzer);
    let lock_cb = Arc::clone(&shared_lock);
    let cmd_topic = topic("command");

    let mut mqtt = EspMqttClient::new_cb(&broker_url, &mqtt_config, move |event| {
        if let EventPayload::Received {
            topic: Some(t),
            data,
            ..
        } = event.payload()
        {
            if t == cmd_topic {
                if let Ok(cmd) = serde_json::from_slice::<CommandPayload>(data) {
                    let mut accepted = true;
                    let mut st = state_cb.lock().unwrap();
                    match cmd.action.as_str() {
                        "relay_on" => {
                            let _ = relay_cb.lock().unwrap().set_high();
                            st.relay = true;
                        }
                        "relay_off" => {
                            let _ = relay_cb.lock().unwrap().set_low();
                            st.relay = false;
                        }
                        "buzzer_on" => {
                            let _ = buzzer_cb.lock().unwrap().set_high();
                            st.buzzer = true;
                        }
                        "buzzer_off" => {
                            let _ = buzzer_cb.lock().unwrap().set_low();
                            st.buzzer = false;
                        }
                        "lock" => {
                            let _ = lock_cb.lock().unwrap().set_high();
                            st.lock = true;
                        }
                        "unlock" => {
                            let _ = lock_cb.lock().unwrap().set_low();
                            st.lock = false;
                        }
                        _ => {
                            accepted = false;
                        }
                    }
                    let result = if accepted { "accepted" } else { "rejected" };
                    let _ = tx_ack.send((cmd.command_id, cmd.action, result));
                }
            }
        }
    })?;

    mqtt.subscribe(&topic("command"), QoS::AtLeastOnce)?;

    let online_payload = serde_json::to_vec(&StatusPayload {
        device_id: DEVICE_ID,
        status: "online",
    })?;
    mqtt.enqueue(&topic("status"), QoS::AtLeastOnce, true, &online_payload)?;

    // Main 5-second telemetry & ACK dispatch loop
    let mut elapsed_ms = 0_u64;
    loop {
        while let Ok((command_id, action, status)) = rx_ack.try_recv() {
            let ack_bytes = serde_json::to_vec(&AckPayload {
                command_id: &command_id,
                device_id: DEVICE_ID,
                action: &action,
                status,
            })?;
            let _ = mqtt.enqueue(&topic("ack"), QoS::AtLeastOnce, false, &ack_bytes);
        }

        if elapsed_ms >= 5000 {
            elapsed_ms = 0;
            let pir = pir_pin.is_high();
            let mq2 = adc.read(&mut mq2_pin).unwrap_or(0);
            let st = *state.lock().unwrap();

            let telemetry = serde_json::to_vec(&SensorPayload {
                device_id: DEVICE_ID,
                pir,
                mq2,
                relay: st.relay,
                buzzer: st.buzzer,
                lock: st.lock,
            })?;
            let _ = mqtt.enqueue(&topic("sensor"), QoS::AtLeastOnce, false, &telemetry);
        }

        thread::sleep(Duration::from_millis(250));
        elapsed_ms += 250;
    }
}
