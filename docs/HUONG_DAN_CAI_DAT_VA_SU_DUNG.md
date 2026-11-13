# Hướng Dẫn Cài Đặt, Phát Triển và Sử Dụng Hệ Thống SmartSecure

Tài liệu này hướng dẫn chi tiết cách cài đặt môi trường, khởi chạy dự án, quy trình làm việc cho **3 thành viên trong nhóm** và cách sử dụng toàn bộ chức năng trên **Web Dashboard SmartSecure (Security Monitoring System)**.

---

## Mục Lục

1. [Tổng quan kiến trúc & Phân công 3 thành viên](#1-tổng-quan-kiến-trúc--phân-công-3-thành-viên)
2. [Công cụ cần cài đặt trước (Prerequisites)](#2-công-cụ-cần-cài-đặt-trước-prerequisites)
3. [Cài đặt & Chạy nhanh toàn hệ thống bằng Docker Compose (Khuyên dùng)](#3-cài-đặt--chạy-nhanh-toàn-hệ-thống-bằng-docker-compose-khuyên-dùng)
4. [Hướng dẫn phát triển riêng cho từng thành viên](#4-hướng-dẫn-phát-triển-riêng-cho-từng-thành-viên)
   - [4.1. Thành viên 1 — Backend, Database, MQTT & Docker](#41-thành-viên-1--backend-database-mqtt--docker)
   - [4.2. Thành viên 2 — Frontend Web Dashboard](#42-thành-viên-2--frontend-web-dashboard)
   - [4.3. Thành viên 3 — AI Service (YOLOv8) & IoT ESP32](#43-thành-viên-3--ai-service-yolov8--iot-esp32)
5. [Hướng dẫn sử dụng Web Dashboard SmartSecure](#5-hướng-dẫn-sử-dụng-web-dashboard-smartsecure)
6. [Quy trình phối hợp Git cho nhóm 3 người](#6-quy-trình-phối-hợp-git-cho-nhóm-3-người)
7. [Xử lý các lỗi thường gặp (Troubleshooting)](#7-xử-lý-các-lỗi-thường-gặp-troubleshooting)

---

## 1. Tổng quan kiến trúc & Phân công 3 thành viên

### 1.1. Danh sách các dịch vụ và cổng (Ports)

| Dịch vụ (Service) | Công nghệ | Cổng (Port) | Địa chỉ truy cập / Giao tiếp |
| :--- | :--- | :---: | :--- |
| **Frontend (Web UI)** | HTML5, CSS3, JS, Nginx | `8080` | <http://localhost:8080> |
| **Backend API** | Python FastAPI, SQLAlchemy | `8000` | <http://localhost:8000> (Swagger: `/docs`) |
| **AI Service** | FastAPI, Ultralytics YOLOv8 | `8001` | <http://localhost:8001/health> |
| **Database** | PostgreSQL 16 | `5432` | `localhost:5432` (DB: `security_monitor`) |
| **MQTT Broker** | Eclipse Mosquitto 2 | `1883` | `mqtt://localhost:1883` |

### 1.2. Phân chia thư mục làm việc theo thành viên

| Thành viên | Vai trò phụ trách | Thư mục làm việc chính | Tài liệu tham chiếu trong `docs/` |
| :--- | :--- | :--- | :--- |
| **Thành viên 1** | Backend, Database, Auth, MQTT Adapter, Docker | `backend/`, `database/`, `docker-compose.yml` | `API_DOCUMENTATION.md`, `DATABASE_DOCUMENTATION.md`, `ARCHITECTURE.md` |
| **Thành viên 2** | Giao diện Web Dashboard, Kết nối REST API & WebSocket | `frontend/` (`index.html`, `css/style.css`, `js/app.js`, `js/api.js`) | `WIREFRAMES_AND_UI_DESIGN.md`, `API_DOCUMENTATION.md` |
| **Thành viên 3** | Nhận diện AI (YOLOv8), Đọc Camera, Firmware ESP32 & Cảm biến | `ai-service/`, `iot-service/`, `models/` | `ai-service/README.md`, `iot-service/README.md` |

---

## 2. Công cụ cần cài đặt trước (Prerequisites)

Trước khi bắt đầu, các thành viên cài đặt các phần mềm sau:

1. **Git**: [Tải Git](https://git-scm.com/) để clone và đồng bộ mã nguồn.
2. **Docker Desktop** *(Bắt buộc nếu muốn chạy toàn bộ hệ thống bằng 1 lệnh)*: [Tải Docker Desktop](https://www.docker.com/products/docker-desktop/). Sau khi cài xong, mở Docker Desktop lên và đợi biểu tượng cá voi chuyển sang trạng thái **Engine running**.
3. **Visual Studio Code (VS Code)**: Trình soạn thảo mã nguồn.
4. **Python 3.12+** *(Tùy chọn cho TV1 và TV3 khi muốn chạy code trực tiếp trên máy không qua Docker)*.
5. **Arduino IDE** *(Dành cho TV3 khi nạp code vào mạch ESP32 thật)*.

---

## 3. Cài đặt & Chạy nhanh toàn hệ thống bằng Docker Compose (Khuyên dùng)

Đây là cách nhanh nhất để **cả 3 thành viên** dựng toàn bộ hệ thống (Database + MQTT + Backend + Frontend + AI Service) trên máy cá nhân.

### Bước 1: Mở Terminal (PowerShell) và di chuyển vào thư mục dự án

```powershell
cd d:\DoAnChuyenNganh\security-monitoring-system
```

### Bước 2: Kiểm tra hoặc tạo file cấu hình `.env`

Nếu chưa có file `.env` ở thư mục gốc, hãy sao chép từ `.env.example`:

```powershell
Copy-Item .env.example .env
```

Mở file `.env` và kiểm tra các thông số (cấu hình mẫu sẵn sàng để chạy Local Dev):

```env
POSTGRES_DB=security_monitor
POSTGRES_USER=security
POSTGRES_PASSWORD=security_dev_pass_2026
SECRET_KEY=smartsecure-local-dev-secret-key-2026-change-in-prod
AI_SERVICE_TOKEN=smartsecure-ai-service-token-2026
BOOTSTRAP_ADMIN_USERNAME=admin
BOOTSTRAP_ADMIN_PASSWORD=Admin@123456
REGISTRATION_ENABLED=true
SEED_DEMO_DATA=true
MODEL_PATH=/models/yolov8n.pt
CONFIDENCE_THRESHOLD=0.6
```

> **Mẹo cho nhóm:**
> - Đặt `SEED_DEMO_DATA=true` nếu muốn hệ thống tự động tạo sẵn dữ liệu mẫu (4 Camera, 2 mạch ESP32, sự kiện AI và cảnh báo mẫu) ngay lần đầu khởi động để tiện kiểm thử giao diện.
> - Đặt `SEED_DEMO_DATA=false` nếu muốn cơ sở dữ liệu trống hoàn toàn và tự thêm mới từ đầu trên giao diện.

### Bước 3: Khởi chạy toàn bộ hệ thống

Chạy lệnh sau tại thư mục gốc của dự án:

```powershell
docker compose up --build -d
```

Kiểm tra xem cả 5 container (`db`, `mqtt`, `backend`, `frontend`, `ai-service`) đã chạy (`Up`) chưa:

```powershell
docker compose ps
```

### Bước 4: Truy cập hệ thống

- **Web Dashboard SmartSecure**: Mở trình duyệt vào **<http://localhost:8080>**
  - **Tài khoản Admin mặc định**:
    - Tên đăng nhập: `admin`
    - Mật khẩu: `Admin@123456` *(hoặc mật khẩu đặt trong `BOOTSTRAP_ADMIN_PASSWORD` ở file `.env`)*
- **Tài liệu Swagger API (Interactive API Docs)**: **<http://localhost:8000/docs>**
- **Kiểm tra trạng thái thực tế của hệ thống (Health Check API)**: **<http://localhost:8000/api/system/health>**
- **Kiểm tra AI Service**: **<http://localhost:8001/health>**

### Các lệnh Docker thường dùng khi làm việc

```powershell
# Xem log trực tiếp của toàn bộ hệ thống (bấm Ctrl+C để thoát xem log)
docker compose logs -f

# Xem log riêng của backend hoặc ai-service
docker compose logs -f backend
docker compose logs -f ai-service

# Khởi động lại 1 dịch vụ sau khi sửa code
docker compose restart backend

# Tắt hệ thống (giữ nguyên dữ liệu Database)
docker compose down

# Tắt hệ thống VÀ xóa sạch dữ liệu Database để làm lại từ đầu (Reset DB)
docker compose down -v
```

---

## 4. Hướng dẫn phát triển riêng cho từng thành viên

### 4.1. Thành viên 1 — Backend, Database, MQTT & Docker

#### A. Chạy Backend trực tiếp bằng Python (ngoài Docker để debug nhanh)

```powershell
cd backend
python -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt

# Thiết lập biến môi trường tạm thời trên PowerShell (dùng SQLite cục bộ hoặc PostgreSQL)
$env:BOOTSTRAP_ADMIN_USERNAME="admin"
$env:BOOTSTRAP_ADMIN_PASSWORD="Admin@123456"
$env:AI_SERVICE_TOKEN="smartsecure-ai-service-token-2026"
$env:SEED_DEMO_DATA="true"

# Chạy server FastAPI với chế độ tự động reload khi sửa code
uvicorn app.main:app --reload --host 0.0.0.0 --port 8000
```

#### B. Cấu trúc code Backend cần nắm

- `backend/app/main.py`: Điểm khởi chạy ứng dụng, tạo bảng tự động, khởi tạo tài khoản Admin (`BOOTSTRAP_ADMIN_*`), nạp dữ liệu mẫu và bật MQTT client.
- `backend/app/models/__init__.py`: Định nghĩa 8 bảng SQLAlchemy (`User`, `Camera`, `Device`, `Sensor`, `DetectionEvent`, `Alert`, `DeviceLog`, `SystemLog`).
- `backend/app/schemas/__init__.py`: Các schema Pydantic kiểm tra dữ liệu đầu vào.
- `backend/app/routers/__init__.py`: Toàn bộ REST API endpoints (`/api/auth/*`, `/api/users/*`, `/api/cameras/*`, `/api/devices/*`, `/api/sensors/*`, `/api/detections/*`, `/api/alerts/*`, `/api/statistics/*`, `/api/system/*`, `/api/logs/*`).
- `backend/app/mqtt/__init__.py`: Lắng nghe dữ liệu cảm biến và phản hồi ACK từ ESP32 qua MQTT.

---

### 4.2. Thành viên 2 — Frontend Web Dashboard

#### A. Cách sửa code Frontend và xem kết quả ngay lập tức

Khi đang chạy hệ thống bằng `docker compose up -d`, thư mục `./frontend` trên máy tính đã được gắn trực tiếp (volume mount) vào container `frontend` (Nginx). Do đó:

1. Mở các file trong thư mục `frontend/` bằng VS Code:
   - `frontend/index.html`: Cấu trúc giao diện 10 màn hình và các hộp thoại (Modal).
   - `frontend/css/style.css`: Giao diện Dark/Light theme, bố cục lưới, thẻ thống kê và hiệu ứng animation chấm trạng thái (`.dot`).
   - `frontend/js/app.js`: Điều hướng chuyển trang, bộ lọc cục bộ, bật/tắt modal.
   - `frontend/js/api.js`: Kết nối toàn bộ REST API, WebSocket thời gian thực, kiểm tra trạng thái hệ thống tự động và phân quyền `ADMIN` / `OPERATOR`.
2. Sau khi lưu file (`Ctrl + S`), chỉ cần quay lại trình duyệt tại **<http://localhost:8080>** và bấm **`Ctrl + F5`** (Hard Reload) là thấy ngay giao diện mới **mà không cần build lại Docker**.

#### B. Chạy thử Frontend độc lập (nếu không bật Docker)

```powershell
cd frontend
python -m http.server 5500
```
*(Lưu ý: Khi chạy bằng `http.server`, nếu muốn gọi API thì nên dùng qua cổng `8080` của Nginx trong Docker để tự động proxy `/api` và `/ws` sang Backend).*

---

### 4.3. Thành viên 3 — AI Service (YOLOv8) & IoT ESP32

#### A. Cài đặt & Chạy thử AI Service (YOLOv8)

1. **Chuẩn bị trọng số model YOLOv8**:
   - Đặt file trọng số `yolov8n.pt` vào thư mục `models/` ở gốc dự án (`d:\DoAnChuyenNganh\security-monitoring-system\models\yolov8n.pt`).
2. **Chạy thử script kiểm tra YOLO trên máy**:
   ```powershell
   cd ai-service
   python -m venv .venv
   .\.venv\Scripts\Activate.ps1
   pip install -r requirements.txt

   # Chạy thử trên ảnh mẫu tự sinh hoặc truyền đường dẫn ảnh thật
   python test_yolo_sample.py
   python test_yolo_sample.py --image duong_dan_anh.jpg
   ```
3. **Giả lập gửi sự kiện phát hiện AI lên Backend (để test cảnh báo nhảy tức thì trên Web)**:
   Mở PowerShell và chạy lệnh sau để giả lập AI vừa phát hiện người xâm nhập tại Camera số 1:
   ```powershell
   Invoke-RestMethod -Uri "http://localhost:8000/api/detections" -Method POST `
     -Headers @{ "Authorization" = "Bearer smartsecure-ai-service-token-2026"; "Content-Type" = "application/json" } `
     -Body '{"camera_id": 1, "object_type": "person", "confidence": 0.95, "event_type": "intrusion", "image_path": "uploads/events/test_intrusion.jpg"}'
   ```
   Ngay sau khi chạy lệnh, Web Dashboard sẽ tự động nhận sự kiện qua WebSocket và tạo cảnh báo **Nghiêm trọng (`CRITICAL`)**!

#### B. Lắp mạch & Nạp code cho ESP32 (`iot-service/`)

1. **Sơ đồ đấu nối chân GPIO trên ESP32 DevKit V1** (khớp với `iot-service/esp32/main.ino`):

   | Linh kiện | Chân Linh kiện | Chân ESP32 | Vai trò |
   | :--- | :--- | :---: | :--- |
   | **Cảm biến chuyển động PIR HC-SR501** | OUT | **GPIO 27** | Phát hiện chuyển động (`INPUT`) |
   | **Cảm biến khí gas/khói MQ-2** | A0 (Analog) | **GPIO 34** | Đo nồng độ khí gas/khói (`ADC1_CH6`) |
   | **Module Relay Kênh 1 (Đèn khu vực)** | IN1 | **GPIO 26** | Bật/tắt đèn (`relay_on` / `relay_off`) |
   | **Còi báo động (Active Buzzer)** | (+) | **GPIO 25** | Bật/tắt còi hú (`buzzer_on` / `buzzer_off`) |
   | **Module Relay Kênh 2 (Khóa điện cửa)** | IN2 | **GPIO 33** | Khóa/mở cửa (`lock` / `unlock`) |

2. **Nạp firmware bằng Arduino IDE**:
   - Cài board **esp32 by Espressif Systems** trong Boards Manager.
   - Cài 2 thư viện trong Library Manager: **`PubSubClient`** và **`ArduinoJson`**.
   - Mở file `iot-service/esp32/main.ino`, sửa các dòng đầu tiên:
     - `WIFI_SSID` và `WIFI_PASSWORD`: Tên và mật khẩu Wi-Fi.
     - `MQTT_HOST`: Địa chỉ IPv4 LAN của máy tính đang chạy Docker (xem bằng lệnh `ipconfig` trên máy tính, ví dụ `192.168.1.10`).
     - `DEVICE_ID`: Mã thiết bị đã tạo trên Web Dashboard (mặc định `ESP32_001`).
   - Cắm cáp USB, chọn cổng COM và nhấn **Upload**.

3. **Giả lập gửi dữ liệu cảm biến ESP32 khi chưa có mạch phần cứng thật**:
   Bạn có thể giả lập mạch `ESP32_001` báo rò rỉ khí gas vượt ngưỡng (`mq2 = 750 ppm`) bằng lệnh PowerShell sau:
   ```powershell
   Invoke-RestMethod -Uri "http://localhost:8000/api/sensors/data" -Method POST `
     -Headers @{ "Content-Type" = "application/json" } `
     -Body '{"device_id": "ESP32_001", "pir": true, "mq2": 750, "relay": true, "buzzer": true, "lock": true}'
   ```

---

## 5. Hướng dẫn sử dụng Web Dashboard SmartSecure

Sau khi mở **<http://localhost:8080>** và đăng nhập, giao diện cung cấp **10 màn hình chức năng** ở thanh menu bên trái:

### 5.1. Ý nghĩa đèn chấm trạng thái (`.dot`) tự động theo thực tế

Hệ thống tự động kiểm tra sức khỏe thực tế của từng dịch vụ/thiết bị định kỳ và hiển thị qua đèn chấm tròn:
- **Chấm xanh lá có sóng lan tỏa (Pulse Animation)**: Dịch vụ hoặc thiết bị đang **Hoạt động / Trực tuyến thực tế** (ví dụ: Backend API, PostgreSQL, MQTT Broker đang chạy, hoặc có Camera/ESP32 đang `ONLINE`).
- **Chấm cam / đỏ nhấp nháy nhanh**: Có cảnh báo an ninh hoặc phát hiện trực tiếp cần chú ý.
- **Chấm xám đứng yên (Không có animation)**: Thành phần đó **Tạm thời chưa hoạt động / Ngoại tuyến** (ví dụ: chưa cắm mạch ESP32, chưa thêm Camera, chưa bật AI Worker hoặc chưa bật WireGuard VPN).

### 5.2. Phân quyền người dùng (`ADMIN` và `OPERATOR`)

| Chức năng | Quản trị viên (`ADMIN`) | Nhân viên vận hành (`OPERATOR`) |
| :--- | :--- | :--- |
| Xem Tổng quan, Giám sát Camera, Giám sát AI, Lịch sử, Trạng thái hệ thống | Có đầy đủ | Có đầy đủ |
| **Xử lý cảnh báo an ninh (`Xử lý`)** | Có | **Có** |
| **Điều khiển thiết bị IoT (Bật/Tắt Đèn, Còi, Khóa cửa)** | Có | **Có** |
| **Thêm / Xóa thiết bị IoT (`ESP32`)** | **Có** | Tự động ẩn nút Thêm/Xóa |
| **Thêm / Sửa / Xóa Camera** | **Có** | Tự động ẩn nút Thêm/Sửa/Xóa (Chỉ xem) |
| **Quản lý người dùng (Thêm tài khoản, Đổi quyền, Khóa/Xóa)** | **Có** | Tự động ẩn mục menu Quản lý người dùng |
| **Cập nhật & Lưu Cài đặt hệ thống** | **Có** | Tự động ẩn nút Lưu cấu hình (Chỉ xem) |

> **Cách kiểm tra phân quyền:**
> 1. Đăng nhập bằng tài khoản `admin`.
> 2. Vào mục **Quản lý người dùng** -> Bấm **Thêm tài khoản** -> Tạo một tài khoản mới với vai trò **Nhân viên vận hành (`OPERATOR`)**.
> 3. Bấm nút **Đăng xuất** ở góc dưới thanh menu trái (hoặc góc trên phải) -> Đăng nhập bằng tài khoản `OPERATOR` vừa tạo để quan sát giao diện tự động ẩn các chức năng quản trị.

### 5.3. Thao tác trên 10 màn hình chính

1. **Tổng quan (`overview`)**: Xem nhanh số lượng Camera trực tuyến, Thiết bị IoT, Sự kiện AI hôm nay, Cảnh báo đang mở, biểu đồ thống kê 7 ngày và xử lý nhanh các cảnh báo mới nhất.
2. **Giám sát Camera (`monitor`)**: Xem lưới hình ảnh các camera an ninh, lọc theo khu vực/trạng thái, chọn bố cục lưới `2x2` / `3x3` và bấm vào từng camera để xem chi tiết.
3. **Giám sát AI (`ai`)**: Theo dõi luồng nhận diện đối tượng YOLOv8 (`Người`, `Phương tiện`, `Xâm nhập`), lọc theo độ tin cậy/mức độ và bấm **Xem** để kiểm tra chi tiết từng sự kiện.
4. **Thiết bị IoT (`iot`)**:
   - Quản trị viên có thể bấm **Thêm thiết bị** để đăng ký mã mạch mới (ví dụ `ESP32_003`).
   - Bấm chọn một thiết bị trong bảng để xem thông số cảm biến `PIR`, nồng độ khí gas `MQ-2 (ppm)` và bấm các nút **Bật/Tắt Đèn (Relay)**, **Bật/Tắt Còi (Buzzer)**, **Khóa/Mở Cửa (Lock)**.
5. **Cảnh báo an ninh (`alerts`)**: Lọc cảnh báo theo mức độ (`CRITICAL`, `WARNING`, `INFO`) hoặc trạng thái (`Chưa xử lý`, `Đã xử lý`). Bấm nút **Xử lý** trên các cảnh báo đang mở để xác nhận đã giải quyết.
6. **Lịch sử sự kiện (`history`)**: Xem dòng thời gian (Timeline) và nhật ký kiểm toán toàn hệ thống (`SystemLog`), hỗ trợ tìm kiếm theo từ khóa và xuất báo cáo CSV. Admins có thể xóa lịch sử nhật ký khi cần dọn dẹp.
7. **Quản lý Camera (`cameras`)**: Thêm camera mới (Tên, Địa chỉ IP, Đường dẫn RTSP, Khu vực lắp đặt, Trạng thái), chỉnh sửa thông tin hoặc xóa camera. *(Mật khẩu trong đường dẫn RTSP được hệ thống tự động ẩn để bảo mật)*.
8. **Quản lý người dùng (`users`)** *(Chỉ dành cho `ADMIN`)*: Thêm tài khoản mới, chuyển đổi vai trò giữa `ADMIN` và `OPERATOR`, khóa/mở khóa hoặc xóa tài khoản.
9. **Trạng thái hệ thống (`system`)**: Kiểm tra trực tiếp tình trạng của 6 thành phần (`Backend API`, `PostgreSQL`, `MQTT & ESP32`, `Camera RTSP`, `AI Worker YOLOv8`, `WireGuard VPN`). Bấm nút **Kiểm tra lại** để quét lại trạng thái tức thì.
10. **Cài đặt hệ thống (`settings`)**: Điều chỉnh ngưỡng báo động khí gas MQ-2 (ppm), ngưỡng độ tin cậy AI tối thiểu (%), thời gian chống trùng lặp cảnh báo (giây) và bấm **Lưu cấu hình**.

---

## 6. Quy trình phối hợp Git cho nhóm 3 người

Để tránh xung đột code (merge conflict) khi cả 3 thành viên cùng làm việc:

1. **Luôn cập nhật code mới nhất trước khi bắt đầu làm việc**:
   ```powershell
   git checkout main
   git pull origin main
   ```
2. **Tạo nhánh (branch) riêng theo tính năng hoặc thành viên**:
   - Thành viên 1: `git checkout -b tv1/backend-update`
   - Thành viên 2: `git checkout -b tv2/frontend-ui`
   - Thành viên 3: `git checkout -b tv3/ai-iot`
3. **Chỉ chỉnh sửa trong phạm vi thư mục mình phụ trách** (TV1 sửa `backend/`, TV2 sửa `frontend/`, TV3 sửa `ai-service/` và `iot-service/`). Nếu cần thay đổi contract API hoặc tên trường JSON, hãy thông báo cho cả nhóm và cập nhật vào `docs/API_DOCUMENTATION.md`.
4. **Commit và đẩy code lên GitHub/GitLab**:
   ```powershell
   git add .
   git commit -m "Mo ta ngan gon thay doi"
   git push origin <ten-nhanh>
   ```
5. **Không commit file nhạy cảm hoặc thư mục nặng**: File `.env`, thư mục `.venv/`, `__pycache__/`, và dữ liệu database cục bộ không nên đưa lên Git (chỉ commit `.env.example`).

---

## 7. Xử lý các lỗi thường gặp (Troubleshooting)

### Lỗi 1: Không đăng nhập được tài khoản `admin` dù gõ đúng mật khẩu trong `.env`
- **Nguyên nhân**: Tài khoản `BOOTSTRAP_ADMIN` chỉ được tạo khi Database khởi tạo lần đầu hoặc chưa có user `admin`. Nếu trước đó bạn đã tạo volume database với mật khẩu cũ, mật khẩu cũ vẫn còn lưu trong DB.
- **Cách khắc phục**: Reset lại volume database rồi bật lại:
  ```powershell
  docker compose down -v
  docker compose up --build -d
  ```

### Lỗi 2: Lỗi `Bind for 0.0.0.0:8080 failed: port is already allocated` (hoặc cổng `5432`, `8000`, `1883`)
- **Nguyên nhân**: Trên máy tính đang có một ứng dụng khác (như PostgreSQL cài sẵn trên Windows hoặc một container cũ) chiếm cổng đó.
- **Cách khắc phục**: Tắt dịch vụ đang chiếm cổng hoặc chạy `docker compose down` ở dự án cũ trước khi chạy lại.

### Lỗi 3: Mạch ESP32 kết nối được Wi-Fi nhưng báo lỗi kết nối MQTT (`failed, rc=-2`)
- **Nguyên nhân**:
  1. `MQTT_HOST` trong `main.ino` đang để `192.168.1.100` (chưa đổi sang IP thật của máy tính chạy Docker).
  2. Tường lửa Windows (Windows Defender Firewall) chặn cổng `1883`.
- **Cách khắc phục**:
  1. Gõ `ipconfig` trên máy tính chạy Docker để lấy đúng `IPv4 Address` rồi điền vào `MQTT_HOST` trong `main.ino`.
  2. Mở cổng `1883` TCP trong Windows Defender Firewall (Inbound Rules).

### Lỗi 4: Sửa code giao diện trong `frontend/` nhưng trình duyệt vẫn hiện giao diện cũ
- **Nguyên nhân**: Trình duyệt lưu bộ nhớ đệm (Cache) cho file `.js` và `.css`.
- **Cách khắc phục**: Bấm tổ hợp phím **`Ctrl + F5`** (hoặc `Ctrl + Shift + R`) trên trình duyệt để tải mới hoàn toàn.
