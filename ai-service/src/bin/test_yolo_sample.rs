use std::env;
use std::fs;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::{json, Value};

use ai_service::{
    camera::{capture_frame_bytes_with_timeout, create_sample_test_image, redact_rtsp_url},
    config::AiConfig,
    detector::{annotate_snapshot, detect, DetectionItem},
    event_handler::{fetch_camera_mappings, send_detection, upload_snapshot},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut image_arg: Option<String> = None;
    let mut rtsp_arg: Option<String> = None;
    let mut save_sample = "sample_frame.jpg".to_string();
    let mut camera_id: Option<i64> = None;
    let mut submit_to_api = false;
    let mut fetch_mapping = false;
    let mut simulate_if_no_model = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--image" if i + 1 < args.len() => {
                image_arg = Some(args[i + 1].clone());
                i += 2;
            }
            "--rtsp" if i + 1 < args.len() => {
                rtsp_arg = Some(args[i + 1].clone());
                i += 2;
            }
            "--save-sample" if i + 1 < args.len() => {
                save_sample = args[i + 1].clone();
                i += 2;
            }
            "--camera-id" if i + 1 < args.len() => {
                camera_id = args[i + 1].parse::<i64>().ok();
                i += 2;
            }
            "--submit" => {
                submit_to_api = true;
                i += 1;
            }
            "--fetch-mapping" => {
                fetch_mapping = true;
                i += 1;
            }
            "--simulate-if-no-model" => {
                simulate_if_no_model = true;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let config = AiConfig::from_env();
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    if fetch_mapping && rtsp_arg.is_none() {
        let cid = camera_id.ok_or("--fetch-mapping yêu cầu truyền thêm --camera-id <id>")?;
        println!("[0/4] Đang lấy ánh xạ RTSP nội bộ cho camera_id={cid} từ Backend...");
        let mappings = fetch_camera_mappings(&http_client, &config).await?;
        let found = mappings
            .into_iter()
            .find(|m| m.camera_id == cid)
            .ok_or_else(|| format!("Không tìm thấy camera_id={cid} trong Backend"))?;
        println!(
            "      -> Tìm thấy camera '{}' ({})",
            found.name,
            redact_rtsp_url(&found.rtsp_url)
        );
        rtsp_arg = Some(found.rtsp_url);
    }

    let (frame_bytes, source_desc) = if let Some(ref rtsp) = rtsp_arg {
        let masked = redact_rtsp_url(rtsp);
        println!(
            "[1/4] Đang chụp frame từ nguồn RTSP: {masked} (timeout {}s) ...",
            config.ffmpeg_timeout_secs
        );
        let bytes = capture_frame_bytes_with_timeout(rtsp, 85, config.ffmpeg_timeout())?;
        (bytes, format!("stream:{masked}"))
    } else if let Some(ref img_path) = image_arg {
        println!("[1/4] Đang đọc ảnh từ file: {img_path} ...");
        let bytes = fs::read(img_path)?;
        (bytes, format!("file:{img_path}"))
    } else {
        println!("[1/4] Không truyền --image/--rtsp, tạo ảnh mẫu thử nghiệm (640x480) ...");
        let bytes = create_sample_test_image(640, 480)?;
        fs::write(&save_sample, &bytes)?;
        (bytes, format!("generated:{save_sample}"))
    };

    println!(
        "      -> Đã nạp frame thành công ({} bytes, nguồn: {}).",
        frame_bytes.len(),
        source_desc
    );

    let model_display = if config.model_path.is_empty() {
        "chưa đặt"
    } else {
        &config.model_path
    };
    println!(
        "[2/4] Suy luận YOLOv8 ONNX (MODEL_PATH={}, CONF={}, NMS_IOU={}) ...",
        model_display, config.confidence_threshold, config.nms_iou_threshold
    );

    let start_t = Instant::now();
    let (detections, mode) = if config.is_onnx_model() {
        let dets = detect(&frame_bytes, &config)?;
        (dets, "yolov8_onnx_fire_smoke")
    } else if simulate_if_no_model {
        println!("      -> Chưa có file .onnx tại MODEL_PATH; dùng --simulate-if-no-model để test luồng PCCC (Lửa/Khói + vẽ bbox + Upload + API).");
        let sim = vec![
            DetectionItem {
                object_type: "fire".to_string(),
                confidence: 0.942,
                event_type: "fire".to_string(),
                bbox: Some([265.0, 255.0, 375.0, 380.0]),
            },
            DetectionItem {
                object_type: "smoke".to_string(),
                confidence: 0.865,
                event_type: "smoke".to_string(),
                bbox: Some([232.0, 92.0, 412.0, 245.0]),
            },
        ];
        (sim, "simulated_pccc_pipeline_test")
    } else {
        return Err(format!(
            "Chưa có model YOLOv8 Lửa & Khói ONNX tại MODEL_PATH='{}'. Hãy đặt MODEL_PATH=/models/yolov8n_fire_smoke.onnx hoặc thêm --simulate-if-no-model để kiểm thử luồng upload & API.",
            config.model_path
        )
        .into());
    };
    let elapsed_ms = start_t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "      -> Hoàn tất suy luận ({:.2} ms), phát hiện {} đối tượng PCCC.",
        elapsed_ms,
        detections.len()
    );

    let mut uploaded_image_path: Option<String> = None;
    let mut api_responses: Vec<Value> = Vec::new();
    let timestamp = Utc::now().to_rfc3339();

    if submit_to_api {
        let cid = camera_id.ok_or("--submit yêu cầu truyền thêm --camera-id <id>")?;
        println!(
            "[3/4] Đang vẽ khung đỏ/vàng, tải snapshot & gửi {} sự kiện PCCC lên Backend ({}) cho camera_id={} ...",
            detections.len(),
            config.backend_url,
            cid
        );
        if !detections.is_empty() {
            let annotated = annotate_snapshot(&frame_bytes, &detections, 88).unwrap_or(frame_bytes.clone());
            let path = upload_snapshot(&http_client, &config, cid, &annotated).await?;
            println!("      -> Đã lưu snapshot tại: {path}");
            uploaded_image_path = Some(path);

            for item in &detections {
                let res = send_detection(
                    &http_client,
                    &config,
                    cid,
                    item,
                    uploaded_image_path.as_deref(),
                    Some(&timestamp),
                )
                .await?;
                api_responses.push(res);
            }
            println!(
                "      -> Đã ghi nhận {} sự kiện vào Backend API.",
                api_responses.len()
            );
        } else {
            println!("      -> Không có đối tượng nào vượt ngưỡng confidence, bỏ qua gửi API.");
        }
    } else {
        println!("[3/4] Chế độ kiểm tra cục bộ (thêm --submit --camera-id <id> để đẩy lên Backend).");
    }

    let report = json!({
        "status": "ok",
        "mode": mode,
        "source": source_desc,
        "camera_id": camera_id,
        "frame_size_bytes": frame_bytes.len(),
        "inference_time_ms": (elapsed_ms * 100.0).round() / 100.0,
        "timestamp": timestamp,
        "image_path": uploaded_image_path,
        "detections_count": detections.len(),
        "detections": detections,
        "api_responses": api_responses,
    });

    println!("[4/4] Kết quả kiểm thử trọn luồng:");
    println!("{}", serde_json::to_string_pretty(&report)?);

    Ok(())
}
