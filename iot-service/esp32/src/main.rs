//! SmartSecure PCCC — Firmware ESP32 viết bằng Rust (`esp-idf-svc` + `esp-idf-hal`).
//!
//! Chức năng: Giám sát khói/gas MQ-2 & cảm biến lửa tại chỗ, đồng thời tiếp nhận lệnh ứng cứu
//! Phòng cháy Chữa cháy qua MQTT Broker (có phản hồi ACK theo `command_id`).
//!
//! Ánh xạ chân GPIO phần cứng PCCC:
//! - Cảm biến Lửa hồng ngoại (Flame Sensor DO)   -> GPIO 27 (Digital Input — trường JSON `pir`)
//! - Cảm biến Khói / Khí gas dễ cháy (MQ-2 A0)   -> GPIO 34 (ADC1_CH6 Analog Input — trường `mq2`)
//! - Mạch Relay kích hoạt (Bơm / Phun sương)     -> GPIO 26 (Digital Output — `relay_on` / `relay_off`)
//! - Còi báo động 12V (qua Relay / MOSFET)       -> GPIO 25 (Digital Output — `buzzer_on` / `buzzer_off`)
//! - Khóa từ Solenoid (Mở cửa thoát hiểm 12V)    -> GPIO 33 (Digital Output — `unlock` / `lock`)

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

// Thông số kết nối VLAN 10 (Camera & IoT hiện trường) tới Mosquitto MQTT Broker ở VLAN 20
const WIFI_SSID: &str = "CHANGE_ME";
const WIFI_PASSWORD: &str = "CHANGE_ME";
const MQTT_HOST: &str = "192.168.1.10";
const MQTT_PORT: u16 = 1883;
const DEVICE_ID: &str = "ESP32_KHO_01";

/// Trạng thái thực tế của 3 cơ cấu chấp hành ứng cứu PCCC trên mạch ESP32.
#[derive(Debug, Clone, Copy)]
struct ActuatorState {
    /// Mạch Relay kích hoạt bơm chữa cháy / van phun sương (`GPIO 26`).
    relay: bool,
    /// Còi báo động 12V kích qua Relay/MOSFET (`GPIO 25`).
    buzzer: bool,
    /// Khóa từ Solenoid cửa thoát hiểm (`GPIO 33`): `true` = đang khóa, `false` = đã nhả mở thoát hiểm.
    lock: bool,
}

impl Default for ActuatorState {
    fn default() -> Self {
        Self {
            relay: false,
            buzzer: false,
            lock: true,
        }
    }
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

/// Bản tin telemetry PCCC gửi lên topic `security/device/{device_id}/sensor` mỗi 5 giây.
#[derive(Debug, Serialize)]
struct SensorPayload<'a> {
    device_id: &'a str,
    /// Ngõ vào số cảm biến Lửa (Flame Sensor GPIO 27; giữ tên trường `pir` đồng bộ schema Backend).
    pir: bool,
    /// Giá trị Analog ADC của cảm biến Khói / Khí dễ cháy MQ-2 (GPIO 34).
    mq2: u16,
    /// Mạch Relay bơm chữa cháy / van phun sương.
    relay: bool,
    /// Còi báo động 12V.
    buzzer: bool,
    /// Khóa từ Solenoid cửa thoát hiểm (`false` = đang mở cửa thoát hiểm khẩn cấp).
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

    // 1. Khởi tạo chân cảm biến PCCC (Flame Sensor + MQ-2 ADC1)
    let flame_pin = PinDriver::input(peripherals.pins.gpio27)?;
    let adc = AdcDriver::new(peripherals.adc1)?;
    let adc_config = AdcChannelConfig {
        attenuation: DB_11,
        ..Default::default()
    };
    let mut mq2_pin = AdcChannelDriver::new(&adc, peripherals.pins.gpio34, &adc_config)?;

    // 2. Khởi tạo chân điều khiển chấp hành PCCC (Relay bơm/phun sương, Còi 12V, Khóa từ Solenoid)
    let mut relay_pin = PinDriver::output(peripherals.pins.gpio26)?;
    let mut buzzer_pin = PinDriver::output(peripherals.pins.gpio25)?;
    let mut solenoid_lock_pin = PinDriver::output(peripherals.pins.gpio33)?;

    // Trạng thái khởi động an toàn: tắt Relay bơm, tắt Còi 12V, cấp giữ Khóa từ Solenoid đóng bình thường
    relay_pin.set_low()?;
    buzzer_pin.set_low()?;
    solenoid_lock_pin.set_high()?;

    let shared_relay = Arc::new(Mutex::new(relay_pin));
    let shared_buzzer = Arc::new(Mutex::new(buzzer_pin));
    let shared_solenoid = Arc::new(Mutex::new(solenoid_lock_pin));
    let state = Arc::new(Mutex::new(ActuatorState::default()));

    // 3. Kết nối mạng Wi-Fi (VLAN Camera/IoT)
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
    log::info!("ESP32 PCCC ({DEVICE_ID}) da ket noi Wi-Fi thanh cong");

    // 4. Kết nối MQTT Broker & đăng ký nhận lệnh ứng cứu PCCC
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
    let solenoid_cb = Arc::clone(&shared_solenoid);
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
                        // Mạch Relay kích hoạt bơm chữa cháy / van phun sương
                        "relay_on" => {
                            let _ = relay_cb.lock().unwrap().set_high();
                            st.relay = true;
                            log::info!("PCCC CMD [relay_on]: Kich hoat Mach Relay bom/phun suong");
                        }
                        "relay_off" => {
                            let _ = relay_cb.lock().unwrap().set_low();
                            st.relay = false;
                            log::info!("PCCC CMD [relay_off]: Ngat Mach Relay bom/phun suong");
                        }
                        // Còi báo động 12V
                        "buzzer_on" => {
                            let _ = buzzer_cb.lock().unwrap().set_high();
                            st.buzzer = true;
                            log::info!("PCCC CMD [buzzer_on]: Bat Coi bao dong 12V");
                        }
                        "buzzer_off" => {
                            let _ = buzzer_cb.lock().unwrap().set_low();
                            st.buzzer = false;
                            log::info!("PCCC CMD [buzzer_off]: Tat Coi bao dong 12V");
                        }
                        // Khóa từ Solenoid cửa thoát hiểm (Fail-Safe: LOW = nhả khóa mở cửa thoát hiểm)
                        "unlock" => {
                            let _ = solenoid_cb.lock().unwrap().set_low();
                            st.lock = false;
                            log::info!("PCCC CMD [unlock]: Nha Khoa tu Solenoid mo cua thoat hiem khan cap");
                        }
                        "lock" => {
                            let _ = solenoid_cb.lock().unwrap().set_high();
                            st.lock = true;
                            log::info!("PCCC CMD [lock]: Khoa lai Khoa tu Solenoid cua thoat hiem");
                        }
                        _ => {
                            accepted = false;
                            log::warn!("PCCC CMD khong ho tro: {}", cmd.action);
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

    // 5. Vòng lặp chính: phản hồi ACK ngay lập tức và gửi telemetry MQ-2 + Flame mỗi 5 giây
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
            let flame_detected = flame_pin.is_high();
            let mq2 = adc.read(&mut mq2_pin).unwrap_or(0);
            let st = *state.lock().unwrap();

            let telemetry = serde_json::to_vec(&SensorPayload {
                device_id: DEVICE_ID,
                pir: flame_detected,
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
