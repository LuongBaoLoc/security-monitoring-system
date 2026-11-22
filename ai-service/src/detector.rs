use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use tract_onnx::prelude::*;

use crate::config::AiConfig;

type YoloPlan = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectionItem {
    pub object_type: String,
    pub confidence: f64,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[f32; 4]>,
}

#[derive(Clone, Debug)]
pub struct LetterboxMeta {
    pub scale: f32,
    pub pad_x: f32,
    pub pad_y: f32,
    pub orig_w: u32,
    pub orig_h: u32,
}

#[derive(Clone, Debug)]
pub struct RawCandidate {
    pub class_id: usize,
    pub confidence: f32,
    pub bbox: [f32; 4],
}

static MODEL_CACHE: OnceLock<Mutex<HashMap<String, Arc<YoloPlan>>>> = OnceLock::new();

/// Chuẩn hóa nhãn PCCC sang loại sự kiện cháy/khói (`fire` | `smoke` | `other`).
pub fn classify_event_type(label: &str) -> String {
    let lower = label.trim().to_ascii_lowercase();
    match lower.as_str() {
        "fire" | "flame" => "fire".to_string(),
        "smoke" => "smoke".to_string(),
        _ => "other".to_string(),
    }
}

/// Ánh xạ chỉ số lớp từ cấu hình `YOLO_CLASSES` (mặc định: `0 => fire`, `1 => smoke`).
pub fn label_for_class(class_id: usize, classes: &[String]) -> String {
    if let Some(name) = classes.get(class_id) {
        return name.clone();
    }
    match class_id {
        0 => "fire".to_string(),
        1 => "smoke".to_string(),
        _ => format!("class_{class_id}"),
    }
}

/// Resize giữ tỷ lệ (letterbox) và chuyển ảnh RGB sang mảng NCHW `[1, 3, size, size]` chuẩn hóa `[0, 1]`.
pub fn letterbox_rgb(img: &RgbImage, target_size: usize) -> (Vec<f32>, LetterboxMeta) {
    let (orig_w, orig_h) = img.dimensions();
    let target_u32 = target_size as u32;
    let scale = (target_size as f32 / orig_w.max(1) as f32)
        .min(target_size as f32 / orig_h.max(1) as f32);
    let new_w = ((orig_w as f32 * scale).round() as u32).clamp(1, target_u32);
    let new_h = ((orig_h as f32 * scale).round() as u32).clamp(1, target_u32);
    let pad_x = (target_u32 - new_w) / 2;
    let pad_y = (target_u32 - new_h) / 2;

    let resized = image::imageops::resize(img, new_w, new_h, FilterType::Triangle);
    let plane = target_size * target_size;
    let pad_val = 114.0_f32 / 255.0_f32;
    let mut tensor_data = vec![pad_val; 3 * plane];

    for y in 0..new_h {
        let dst_y = (y + pad_y) as usize;
        for x in 0..new_w {
            let dst_x = (x + pad_x) as usize;
            let px = resized.get_pixel(x, y);
            let idx = dst_y * target_size + dst_x;
            tensor_data[idx] = px[0] as f32 / 255.0;
            tensor_data[plane + idx] = px[1] as f32 / 255.0;
            tensor_data[2 * plane + idx] = px[2] as f32 / 255.0;
        }
    }

    let meta = LetterboxMeta {
        scale,
        pad_x: pad_x as f32,
        pad_y: pad_y as f32,
        orig_w,
        orig_h,
    };
    (tensor_data, meta)
}

/// Tính chỉ số giao trên hợp (IoU) giữa 2 khung `[x1, y1, x2, y2]`.
pub fn compute_iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let inter_x1 = a[0].max(b[0]);
    let inter_y1 = a[1].max(b[1]);
    let inter_x2 = a[2].min(b[2]);
    let inter_y2 = a[3].min(b[3]);
    let inter_w = (inter_x2 - inter_x1).max(0.0);
    let inter_h = (inter_y2 - inter_y1).max(0.0);
    let inter_area = inter_w * inter_h;
    if inter_area <= 0.0 {
        return 0.0;
    }
    let area_a = (a[2] - a[0]).max(0.0) * (a[3] - a[1]).max(0.0);
    let area_b = (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0);
    let union = area_a + area_b - inter_area;
    if union <= 0.0 {
        0.0
    } else {
        inter_area / union
    }
}

/// Non-Maximum Suppression theo từng lớp PCCC (loại bỏ các khung trùng lặp có IoU > ngưỡng).
pub fn apply_nms(mut candidates: Vec<RawCandidate>, iou_threshold: f32) -> Vec<RawCandidate> {
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut kept: Vec<RawCandidate> = Vec::new();
    for cand in candidates {
        let suppressed = kept
            .iter()
            .any(|k| k.class_id == cand.class_id && compute_iou(&k.bbox, &cand.bbox) > iou_threshold);
        if !suppressed {
            kept.push(cand);
        }
    }
    kept
}

/// Giải mã tensor đầu ra YOLOv8 chuyên PCCC (`[1, 4+C, N]` hoặc `[1, N, 4+C]`, vd 2 lớp fire/smoke -> `[1, 6, 8400]`).
pub fn postprocess_yolov8_output(
    flat_data: &[f32],
    dim1: usize,
    dim2: usize,
    meta: &LetterboxMeta,
    classes: &[String],
    conf_threshold: f32,
    iou_threshold: f32,
) -> Vec<DetectionItem> {
    if dim1 < 5 && dim2 < 5 {
        return Vec::new();
    }

    // YOLOv8 chuẩn xuất ra [1, 4+C, 8400] (channels_first = true) hoặc [1, 8400, 4+C] (false).
    let channels_first = if dim2 < 5 && dim1 >= 5 {
        true
    } else if dim1 < 5 && dim2 >= 5 {
        false
    } else {
        dim1 <= dim2
    };
    let (num_features, num_anchors) = if channels_first {
        (dim1, dim2)
    } else {
        (dim2, dim1)
    };
    if num_features <= 4 || flat_data.len() < num_features * num_anchors {
        return Vec::new();
    }

    let get_val = |feat: usize, anchor: usize| -> f32 {
        if channels_first {
            flat_data[feat * num_anchors + anchor]
        } else {
            flat_data[anchor * num_features + feat]
        }
    };

    let mut candidates = Vec::new();
    let max_w = meta.orig_w as f32;
    let max_h = meta.orig_h as f32;

    for anchor in 0..num_anchors {
        let mut best_cls = 0usize;
        let mut best_score = 0.0_f32;
        for c in 4..num_features {
            let score = get_val(c, anchor);
            if score > best_score {
                best_score = score;
                best_cls = c - 4;
            }
        }
        if best_score < conf_threshold {
            continue;
        }

        let cx = get_val(0, anchor);
        let cy = get_val(1, anchor);
        let bw = get_val(2, anchor);
        let bh = get_val(3, anchor);

        let x1 = ((cx - bw * 0.5 - meta.pad_x) / meta.scale).clamp(0.0, max_w);
        let y1 = ((cy - bh * 0.5 - meta.pad_y) / meta.scale).clamp(0.0, max_h);
        let x2 = ((cx + bw * 0.5 - meta.pad_x) / meta.scale).clamp(0.0, max_w);
        let y2 = ((cy + bh * 0.5 - meta.pad_y) / meta.scale).clamp(0.0, max_h);

        if x2 - x1 < 1.0 || y2 - y1 < 1.0 {
            continue;
        }

        candidates.push(RawCandidate {
            class_id: best_cls,
            confidence: best_score,
            bbox: [
                (x1 * 10.0).round() / 10.0,
                (y1 * 10.0).round() / 10.0,
                (x2 * 10.0).round() / 10.0,
                (y2 * 10.0).round() / 10.0,
            ],
        });
    }

    let final_boxes = apply_nms(candidates, iou_threshold);
    final_boxes
        .into_iter()
        .map(|item| {
            let label = label_for_class(item.class_id, classes);
            let event_type = classify_event_type(&label);
            let conf = ((item.confidence as f64) * 10000.0).round() / 10000.0;
            DetectionItem {
                object_type: label,
                confidence: conf.clamp(0.0, 1.0),
                event_type,
                bbox: Some(item.bbox),
            }
        })
        .collect()
}

/// Vẽ khung bounding box PCCC lên ảnh snapshot (Đỏ tươi cho Lửa `fire`, Vàng cam cho Khói `smoke`).
pub fn annotate_snapshot(
    image_bytes: &[u8],
    detections: &[DetectionItem],
    jpeg_quality: u8,
) -> Result<Vec<u8>, String> {
    let mut img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("Invalid image for annotation: {e}"))?
        .to_rgb8();

    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Ok(image_bytes.to_vec());
    }

    for det in detections {
        let Some([x1, y1, x2, y2]) = det.bbox else {
            continue;
        };
        let color = match det.event_type.as_str() {
            "fire" => Rgb([239, 68, 68]),    // Đỏ tươi cảnh báo Lửa
            "smoke" => Rgb([245, 158, 11]),  // Vàng cam cảnh báo Khói
            _ => Rgb([59, 130, 246]),
        };

        let left = (x1.max(0.0) as u32).min(w - 1);
        let top = (y1.max(0.0) as u32).min(h - 1);
        let right = (x2.max(0.0) as u32).min(w - 1);
        let bottom = (y2.max(0.0) as u32).min(h - 1);
        if right <= left || bottom <= top {
            continue;
        }

        // Vẽ viền khung dày 3px rõ nét trên ảnh bằng chứng
        draw_rect_outline(&mut img, left, top, right, bottom, 3, color);
        // Vẽ thanh nhãn màu nổi ở mép trên khung
        let tag_top = top.saturating_sub(14);
        let tag_right = (left + 72).min(right);
        draw_filled_rect(&mut img, left, tag_top, tag_right, top, color);
    }

    let mut out = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut out, jpeg_quality);
    encoder
        .encode_image(&DynamicImage::ImageRgb8(img))
        .map_err(|e| format!("Failed to encode annotated JPEG snapshot: {e}"))?;
    Ok(out.into_inner())
}

fn draw_rect_outline(
    img: &mut RgbImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    thickness: u32,
    color: Rgb<u8>,
) {
    let (w, h) = img.dimensions();
    for t in 0..thickness {
        let top = (y0 + t).min(h - 1);
        let bot = y1.saturating_sub(t).min(h - 1);
        for x in x0..=x1.min(w - 1) {
            img.put_pixel(x, top, color);
            img.put_pixel(x, bot, color);
        }
        let left = (x0 + t).min(w - 1);
        let right = x1.saturating_sub(t).min(w - 1);
        for y in y0..=y1.min(h - 1) {
            img.put_pixel(left, y, color);
            img.put_pixel(right, y, color);
        }
    }
}

fn draw_filled_rect(img: &mut RgbImage, x0: u32, y0: u32, x1: u32, y1: u32, color: Rgb<u8>) {
    let (w, h) = img.dimensions();
    for y in y0.min(h)..=y1.min(h - 1) {
        for x in x0.min(w)..=x1.min(w - 1) {
            img.put_pixel(x, y, color);
        }
    }
}

fn load_or_get_onnx_model(model_path: &str, input_size: usize) -> Result<Arc<YoloPlan>, String> {
    let cache = MODEL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cache_key = format!("{model_path}@{input_size}");

    if let Ok(guard) = cache.lock() {
        if let Some(plan) = guard.get(&cache_key) {
            return Ok(Arc::clone(plan));
        }
    }

    let path = Path::new(model_path);
    if !path.is_file() {
        return Err(format!("Model file not found at MODEL_PATH={model_path}"));
    }
    let is_onnx = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("onnx"))
        .unwrap_or(false);
    if !is_onnx {
        return Err(format!(
            "AI Service PCCC yêu cầu model định dạng ONNX (.onnx). File hiện tại '{model_path}' không phải .onnx (hãy xuất bằng: yolo export model=yolov8n_fire_smoke.pt format=onnx imgsz={input_size})."
        ));
    }

    let runnable = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|e| format!("Failed to parse ONNX model '{model_path}': {e}"))?
        .with_input_fact(
            0,
            InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, input_size, input_size)),
        )
        .map_err(|e| {
            format!("Failed to set ONNX input shape [1,3,{input_size},{input_size}]: {e}")
        })?
        .into_optimized()
        .map_err(|e| format!("Failed to optimize ONNX graph: {e}"))?
        .into_runnable()
        .map_err(|e| format!("Failed to build runnable ONNX plan: {e}"))?;

    let arc_plan = Arc::new(runnable);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(cache_key, Arc::clone(&arc_plan));
    }
    Ok(arc_plan)
}

/// Chạy suy luận YOLOv8 ONNX chuyên Lửa (`fire`) & Khói (`smoke`) trên ảnh đầu vào.
pub fn detect(image_bytes: &[u8], config: &AiConfig) -> Result<Vec<DetectionItem>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("Invalid image: {e}"))?
        .to_rgb8();

    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("Image has zero width or height".to_string());
    }

    if !config.is_model_configured() {
        return Err(
            "Chưa cấu hình model YOLOv8 Lửa & Khói (MODEL_PATH trống hoặc chưa có file .onnx)"
                .to_string(),
        );
    }

    let input_size = config.input_size;
    let model = load_or_get_onnx_model(&config.model_path, input_size)?;
    let (nchw_data, meta) = letterbox_rgb(&img, input_size);

    let input_tensor = Tensor::from_shape(&[1, 3, input_size, input_size], &nchw_data)
        .map_err(|e| format!("Failed to build input tensor: {e}"))?;

    let outputs = model
        .run(tvec!(input_tensor.into()))
        .map_err(|e| format!("YOLOv8 ONNX inference failed: {e}"))?;

    let output_tensor = outputs
        .first()
        .ok_or_else(|| "ONNX model produced no output tensors".to_string())?;
    let view = output_tensor
        .to_array_view::<f32>()
        .map_err(|e| format!("Failed to read float32 output tensor: {e}"))?;

    let shape = view.shape();
    let (dim1, dim2) = match shape {
        [_, d1, d2] => (*d1, *d2),
        [d1, d2] => (*d1, *d2),
        _ => {
            return Err(format!(
                "Unsupported YOLOv8 output tensor shape: {shape:?}"
            ));
        }
    };

    let flat: Vec<f32> = view.iter().copied().collect();
    Ok(postprocess_yolov8_output(
        &flat,
        dim1,
        dim2,
        &meta,
        &config.yolo_classes,
        config.confidence_threshold as f32,
        config.nms_iou_threshold,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::create_sample_test_image;

    #[test]
    fn nms_filters_fire_and_smoke_detections() {
        let meta = LetterboxMeta {
            scale: 1.0,
            pad_x: 0.0,
            pad_y: 0.0,
            orig_w: 640,
            orig_h: 640,
        };
        let classes = vec!["fire".to_string(), "smoke".to_string()];
        // Shape [1, 6, 3]: 4 tọa độ + 2 lớp PCCC (fire=0, smoke=1), 3 anchor
        // Anchor 0: fire (0.94) tại (320, 310, 90, 110)
        // Anchor 1: fire trùng lặp (0.81) tại (324, 312, 90, 110) -> bị NMS loại bỏ
        // Anchor 2: smoke (0.87) tại (330, 160, 140, 120) -> được giữ lại
        let flat = vec![
            320.0, 324.0, 330.0, // cx
            310.0, 312.0, 160.0, // cy
            90.0, 90.0, 140.0,   // w
            110.0, 110.0, 120.0, // h
            0.94, 0.81, 0.08,    // class 0 (fire)
            0.05, 0.04, 0.87,    // class 1 (smoke)
        ];
        let results = postprocess_yolov8_output(&flat, 6, 3, &meta, &classes, 0.6, 0.45);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].object_type, "fire");
        assert_eq!(results[0].event_type, "fire");
        assert_eq!(results[1].object_type, "smoke");
        assert_eq!(results[1].event_type, "smoke");
    }

    #[test]
    fn annotates_fire_and_smoke_boxes_on_jpeg_snapshot() {
        let raw = create_sample_test_image(640, 480).expect("sample pccc image");
        let dets = vec![
            DetectionItem {
                object_type: "fire".to_string(),
                confidence: 0.942,
                event_type: "fire".to_string(),
                bbox: Some([265.0, 245.0, 375.0, 375.0]),
            },
            DetectionItem {
                object_type: "smoke".to_string(),
                confidence: 0.865,
                event_type: "smoke".to_string(),
                bbox: Some([230.0, 95.0, 415.0, 250.0]),
            },
        ];
        let annotated = annotate_snapshot(&raw, &dets, 88).expect("annotated jpeg");
        assert!(annotated.starts_with(&[0xFF, 0xD8, 0xFF]));
        assert!(annotated.len() > 2000);
    }
}
