use serde_json::json;
use std::env;
use std::fs;
use std::time::Instant;

use ai_service::{
    camera::{capture_frame_bytes, create_sample_test_image},
    config::AiConfig,
    detector::Detector,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut image_arg: Option<String> = None;
    let mut rtsp_arg: Option<String> = None;
    let mut save_sample = "sample_frame.jpg".to_string();

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
            _ => {
                i += 1;
            }
        }
    }

    let (frame_bytes, source_desc) = if let Some(ref rtsp) = rtsp_arg {
        println!("[1/3] Đang đọc frame từ camera RTSP ...");
        let bytes = capture_frame_bytes(rtsp, 85)?;
        (bytes, "RTSP".to_string())
    } else if let Some(ref img_path) = image_arg {
        println!("[1/3] Đang đọc ảnh mẫu từ file: {img_path} ...");
        let bytes = fs::read(img_path)?;
        (bytes, format!("file:{img_path}"))
    } else {
        println!("[1/3] Không truyền --image/--rtsp, đang tạo ảnh mẫu thử nghiệm (640x480) ...");
        let bytes = create_sample_test_image(640, 480)?;
        fs::write(&save_sample, &bytes)?;
        (bytes, format!("generated:{save_sample}"))
    };

    println!(
        "      -> Đã nạp frame thành công ({} bytes, nguồn: {}).",
        frame_bytes.len(),
        source_desc
    );

    let config = AiConfig::from_env();
    let mut detector = Detector::load(&config)?;
    println!(
        "[2/3] YOLOv8 ONNX {:?}, ngưỡng {}",
        detector.models(),
        config.confidence_threshold
    );

    let start_t = Instant::now();
    let detections = detector
        .detect(&frame_bytes)
        .map_err(|error| format!("{error:?}"))?;
    let elapsed_ms = start_t.elapsed().as_secs_f64() * 1000.0;

    let report = json!({
        "status": "ok",
        "mode": "yolov8_onnx",
        "source": source_desc,
        "frame_size_bytes": frame_bytes.len(),
        "inference_time_ms": (elapsed_ms * 100.0).round() / 100.0,
        "detections_count": detections.len(),
        "detections": detections,
    });

    println!("[3/3] Kết quả chạy thử YOLO:");
    println!("{}", serde_json::to_string_pretty(&report)?);

    Ok(())
}
