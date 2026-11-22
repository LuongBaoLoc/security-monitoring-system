# ESP32 & Thiết bị IoT Ứng cứu PCCC (Rust Firmware — `esp-idf-svc` & `esp-idf-hal`)

Thư mục này chứa mã nguồn firmware **Rust** [`esp32/src/main.rs`](esp32/src/main.rs) và sơ đồ đấu nối phần cứng cho nút cảm biến khói/lửa và điều khiển ứng cứu **Phòng cháy Chữa cháy (PCCC)** trên vi điều khiển **ESP32-WROOM-32 DevKit V1**.

---

## 1. Danh sách Linh kiện Phần cứng PCCC (Tuần 2 – Tuần 3)

| STT | Linh kiện | Mã / Thông số | Vai trò PCCC trong hệ thống |
| :---: | :--- | :--- | :--- |
| 1 | **Vi điều khiển** | ESP32-WROOM-32 DevKit V1 | Kết nối Wi-Fi (VLAN 10), giao tiếp MQTT Broker, đọc cảm biến & phản hồi `ACK` |
| 2 | **Cảm biến khói / khí gas** | **MQ-2** (Analog A0) | Đo nồng độ khói & khí dễ cháy (LPG, metan, khói cháy) qua ADC 12-bit (`GPIO 34`) |
| 3 | **Cảm biến lửa hồng ngoại** | **Flame Sensor (IR)** (Digital DO) | Phát hiện bức xạ tia lửa trực tiếp tại khu vực giám sát (`GPIO 27`) |
| 4 | **Mạch Relay kích hoạt** | Module Relay 5V cách ly quang (Opto-isolated) | Đóng/ngắt **bơm chữa cháy mini / van phun sương** (`relay_on` / `relay_off`, `GPIO 26`) |
| 5 | **Còi báo động 12V** | Còi hú báo cháy 12V (kích qua Relay / MOSFET) | Phát tín hiệu báo cháy công suất lớn (`buzzer_on` / `buzzer_off`, `GPIO 25`) |
| 6 | **Khóa từ Solenoid** | Khóa điện từ 12V (chế độ Fail-Safe qua Relay) | **Nhả khóa mở cửa thoát hiểm khẩn cấp** khi xảy ra cháy (`unlock` / `lock`, `GPIO 33`) |
| 7 | **Nguồn cấp cách ly** | Adapter 5V-2A (ESP32 + MQ-2) & Nguồn 12V-3A (Còi 12V + Solenoid + Bơm) | Đảm bảo nguồn ổn định, chống sụt áp reset ESP32 khi đóng cắt tải 12V |

---

## 2. Sơ đồ Đấu nối Chân GPIO (Pinout — Khớp 100% `esp32/src/main.rs`)

| Thiết bị ngoại vi PCCC | Chân linh kiện | Chân ESP32 | Chế độ GPIO | Lệnh MQTT / Trường JSON |
| :--- | :--- | :---: | :--- | :--- |
| **Cảm biến Lửa (Flame IR)** | VCC / DO / GND | 3.3V / **GPIO 27** / GND | `INPUT` (`PinDriver::input`) | Trường `pir` (tín hiệu số báo lửa tại chỗ) |
| **Cảm biến Khói/Gas MQ-2** | VCC / A0 / GND | 5V / **GPIO 34 (ADC1_CH6)** | `INPUT` (`AdcChannelDriver`) | Trường `mq2` (nồng độ khói/gas, dùng ADC1 không xung đột Wi-Fi) |
| **Mạch Relay (Bơm / Phun sương)** | VCC / IN1 / GND | 5V / **GPIO 26** / GND | `OUTPUT` (`PinDriver::output`) | `relay_on` / `relay_off` (trường `relay`) |
| **Còi báo động 12V (qua Relay/FET)** | Control / GND | **GPIO 25** / GND | `OUTPUT` (`PinDriver::output`) | `buzzer_on` / `buzzer_off` (trường `buzzer`) |
| **Khóa từ Solenoid (Thoát hiểm)** | VCC / IN2 / GND | 5V / **GPIO 33** / GND | `OUTPUT` (`PinDriver::output`) | `unlock` (mở thoát hiểm) / `lock` (trường `lock`) |

```text
                           +----------------------------------+
   Flame Sensor (DO)  ---> | GPIO 27                  GPIO 26 | ---> Relay 1 (Bom chua chay / Phun suong)
   MQ-2 Khoi/Gas (A0) ---> | GPIO 34 (ADC1)           GPIO 25 | ---> Relay/FET -> Coi bao dong 12V
   Nguon 5V / GND     ---> | VIN / GND                GPIO 33 | ---> Relay 2 -> Khoa tu Solenoid (Thoat hiem)
                           |        ESP32-WROOM-32            |
                           +----------------------------------+
```

> **Lưu ý an toàn đấu nối tải 12V:** Còi báo động 12V, Khóa từ Solenoid 12V và máy bơm chữa cháy **không cấp trực tiếp từ chân GPIO 3.3V của ESP32** mà phải đi qua mạch Relay cách ly quang hoặc MOSFET có diode chống dòng ngược (flyback diode).

---

## 3. Giao thức MQTT & Topics

- `security/device/{device_id}/sensor`: Gửi telemetry định kỳ mỗi 5 giây (`mq2`, tín hiệu lửa `pir`, trạng thái `relay`, `buzzer`, `lock`).
- `security/device/{device_id}/status`: Gửi trạng thái `online` khi kết nối Broker thành công.
- `security/device/{device_id}/command`: Nhận lệnh điều khiển ứng cứu PCCC (`relay_on`, `relay_off`, `buzzer_on`, `buzzer_off`, `unlock`, `lock`) kèm `command_id`.
- `security/device/{device_id}/ack`: Gửi phản hồi xác nhận (`accepted` / `rejected`) kèm `command_id` sau khi điều khiển GPIO thành công.

---

## 4. Hướng dẫn Biên dịch & Nạp Firmware Rust cho ESP32

1. Cài toolchain Rust cho ESP32:
   ```powershell
   cargo install espup espflash ldproxy
   espup install
   ```
2. Cập nhật `WIFI_SSID`, `WIFI_PASSWORD`, `MQTT_HOST` (IP máy chủ Docker/Mosquitto) và `DEVICE_ID` (ví dụ `ESP32_KHO_01`) trong [`esp32/src/main.rs`](esp32/src/main.rs).
3. Biên dịch và nạp vào mạch qua cổng USB/COM:
   ```powershell
   cd iot-service/esp32
   cargo build --release
   espflash flash --monitor target/xtensa-esp32-espidf/release/smartsecure-esp32-firmware
   ```
