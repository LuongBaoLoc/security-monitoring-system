# ESP32 & Cảm biến IoT (Thành viên 3)

Thư mục này chứa mã nguồn firmware mẫu `esp32/main.ino` và sơ đồ đấu nối phần cứng cho nút cảm biến/điều khiển an ninh dùng vi điều khiển **ESP32 DevKit V1**.

## 1. Danh sách phần cứng (Tuần 2 – Tuần 3)

| STT | Linh kiện | Mã / Thông số | Vai trò trong hệ thống |
|---|---|---|---|
| 1 | Vi điều khiển | ESP32-WROOM-32 DevKit V1 | Kết nối Wi-Fi, giao tiếp MQTT, đọc cảm biến & điều khiển chấp hành |
| 2 | Cảm biến chuyển động | PIR HC-SR501 | Phát hiện chuyển động người trong vùng quét (tín hiệu Digital HIGH/LOW) |
| 3 | Cảm biến khí gas / khói | MQ-2 | Đo nồng độ khí dễ cháy / khói qua ngõ Analog (ADC 12-bit trên ESP32) |
| 4 | Module Relay 2 kênh | 5V Opto-isolated Relay | Đóng/ngắt đèn chiếu sáng cảnh báo (`relay`) và khóa điện từ (`lock`) |
| 5 | Còi báo động | Active Buzzer 5V (hoặc qua Transistor/Relay) | Phát âm thanh cảnh báo tại chỗ (`buzzer`) |
| 6 | Nguồn cấp | Adapter 5V – 2A (và 12V cho khóa điện từ) | Cấp nguồn ổn định cho ESP32, module relay và cảm biến MQ-2 |

## 2. Sơ đồ đấu nối chân (Pinout / Wiring Diagram – Tuần 3)

Bảng ánh xạ chân GPIO trên ESP32 khớp trực tiếp với firmware [`esp32/main.ino`](esp32/main.ino):

| Thiết bị ngoại vi | Chân linh kiện | Chân ESP32 | Chế độ GPIO | Lưu ý kỹ thuật |
|---|---|---|---|---|
| **PIR HC-SR501** | VCC / OUT / GND | VIN (5V) / **GPIO 27** / GND | `INPUT` | Chân OUT của HC-SR501 xuất mức logic 3.3V an toàn cho ESP32 |
| **Cảm biến MQ-2** | VCC / A0 / GND | VIN (5V) / **GPIO 34 (ADC1_CH6)** / GND | `INPUT` (Analog) | Dùng ADC1 (GPIO 34) vì ADC2 xung đột khi bật Wi-Fi; dùng cầu phân áp nếu A0 > 3.3V |
| **Relay Đèn khu vực** | VCC / IN1 / GND | VIN (5V) / **GPIO 26** / GND | `OUTPUT` | Điều khiển qua lệnh MQTT `relay_on` / `relay_off` |
| **Còi báo động (Buzzer)** | (+) / (-) | **GPIO 25** / GND | `OUTPUT` | Điều khiển qua lệnh MQTT `buzzer_on` / `buzzer_off` |
| **Relay Khóa điện cửa** | VCC / IN2 / GND | VIN (5V) / **GPIO 33** / GND | `OUTPUT` | Điều khiển qua lệnh MQTT `lock` / `unlock` |

```text
                       +-----------------------+
     PIR HC-SR501 ---> | GPIO 27       GPIO 26 | ---> Relay 1 (Đèn khu vực)
     MQ-2 (A0)    ---> | GPIO 34 (ADC) GPIO 25 | ---> Buzzer (Còi báo động)
     5V / GND     ---> | VIN / GND     GPIO 33 | ---> Relay 2 (Khóa điện)
                       |     ESP32 DevKit      |
                       +-----------------------+
```

## 3. Giao thức MQTT & Topics

- `security/device/{device_id}/sensor`: ESP32 gửi định kỳ mỗi 5 giây (`pir`, `mq2`, `relay`, `buzzer`, `lock`).
- `security/device/{device_id}/status`: ESP32 gửi trạng thái `online` khi kết nối thành công.
- `security/device/{device_id}/command`: ESP32 lắng nghe lệnh điều khiển từ Backend (`relay_on`, `relay_off`, `buzzer_on`, `buzzer_off`, `lock`, `unlock`).
- `security/device/{device_id}/ack`: ESP32 gửi xác nhận (`accepted` / `rejected`) kèm `command_id`.

## 4. Hướng dẫn nạp firmware

1. Cài đặt **Arduino IDE** và gói board **esp32 by Espressif Systems**.
2. Trong Library Manager, cài đặt 2 thư viện: **`PubSubClient`** và **`ArduinoJson`**.
3. Mở file `esp32/main.ino`, cập nhật `WIFI_SSID`, `WIFI_PASSWORD`, `MQTT_HOST` và `DEVICE_ID` khớp với `device_code` đã đăng ký trong bảng `devices`.
4. Chọn đúng cổng COM và nhấn **Upload**.
