use std::{env, io::Cursor, path::Path};

use image::{imageops::FilterType, ImageReader, Limits, RgbImage};
use ort::{session::Session, value::Tensor};
use serde::Serialize;

use crate::config::AiConfig;

#[derive(Clone, Debug, Serialize)]
pub struct DetectionItem {
    pub object_type: String,
    pub confidence: f64,
    pub event_type: String,
}

#[derive(Clone, Copy)]
enum ModelKind {
    FireSmoke,
    Security,
}

impl ModelKind {
    fn name(self) -> &'static str {
        match self {
            Self::FireSmoke => "fire_smoke",
            Self::Security => "security",
        }
    }

    fn classes(self) -> usize {
        match self {
            Self::FireSmoke => 2,
            Self::Security => 80,
        }
    }

    fn input_size(self) -> usize {
        match self {
            Self::FireSmoke => 768,
            Self::Security => 640,
        }
    }

    fn label(self, class_id: usize) -> Option<&'static str> {
        match (self, class_id) {
            (Self::FireSmoke, 0) => Some("fire"),
            (Self::FireSmoke, 1) => Some("smoke"),
            (Self::Security, 0) => Some("person"),
            (Self::Security, 2) => Some("car"),
            (Self::Security, 3) => Some("motorcycle"),
            (Self::Security, 5) => Some("bus"),
            (Self::Security, 7) => Some("truck"),
            _ => None,
        }
    }
}

struct Model {
    session: Session,
    height: usize,
    width: usize,
    kind: ModelKind,
}

impl Model {
    fn load(path: &str, kind: ModelKind) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|error| error.to_string())?
            .with_intra_threads(2)
            .map_err(|error| error.to_string())?
            .commit_from_file(path)
            .map_err(|error| error.to_string())?;
        let ([input], [output]) = (session.inputs(), session.outputs()) else {
            return Err("Expected one input and one output for YOLOv8 detection".into());
        };
        let input_shape = input
            .dtype()
            .tensor_shape()
            .ok_or("Expected tensor input")?;
        let output_shape = output
            .dtype()
            .tensor_shape()
            .ok_or("Expected tensor output")?;
        let [batch, 3, height, width] = &input_shape[..] else {
            return Err("Expected YOLOv8 image input in NCHW RGB format".into());
        };
        if !matches!(*batch, -1 | 1) {
            return Err("Expected a single-image input batch".into());
        }
        let fields = (kind.classes() + 4) as i64;
        if !matches!(&output_shape[..], [_, a, b] if *a == fields || *b == fields || *a == -1 || *b == -1)
        {
            return Err("Unexpected YOLOv8 detection output shape or class count".into());
        }
        let dimension = |value: i64| match value {
            -1 => Ok(kind.input_size()),
            1..=2048 => Ok(value as usize),
            _ => Err("Unsupported model input size"),
        };
        let height = dimension(*height)?;
        let width = dimension(*width)?;
        Ok(Self {
            session,
            height,
            width,
            kind,
        })
    }

    fn predict(&mut self, image: &RgbImage, threshold: f32) -> Result<Vec<DetectionItem>, String> {
        let input = letterbox(image, self.width, self.height);
        let tensor = Tensor::from_array(([1, 3, self.height, self.width], input))
            .map_err(|error| error.to_string())?;
        let outputs = self
            .session
            .run(ort::inputs![tensor])
            .map_err(|error| error.to_string())?;
        let (shape, values) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|error| error.to_string())?;
        decode_predictions(&shape[..], values, self.kind, threshold)
    }
}

pub struct Detector {
    models: Vec<Model>,
    threshold: f32,
}

#[derive(Debug)]
pub enum DetectionError {
    InvalidImage,
    Inference(String),
}

impl Detector {
    pub fn load(config: &AiConfig) -> Result<Self, String> {
        let runtime = env::var("ORT_DYLIB_PATH").unwrap_or_else(|_| {
            if cfg!(target_os = "windows") {
                "onnxruntime.dll".to_string()
            } else if cfg!(target_os = "macos") {
                "libonnxruntime.dylib".to_string()
            } else {
                "libonnxruntime.so".to_string()
            }
        });
        ort::init_from(runtime)
            .map_err(|error| error.to_string())?
            .commit();

        let paths = [
            (ModelKind::FireSmoke, &config.fire_model_path),
            (ModelKind::Security, &config.security_model_path),
        ];
        let mut models = Vec::new();
        for (kind, path) in paths {
            if path.is_empty() {
                continue;
            }
            if !Path::new(path).is_file() {
                tracing::warn!(model = kind.name(), "ONNX model file is missing");
                continue;
            }
            match Model::load(path, kind) {
                Ok(model) => models.push(model),
                Err(error) => {
                    tracing::warn!(model = kind.name(), %error, "Unable to load ONNX model")
                }
            }
        }
        if models.is_empty() {
            return Err("No usable YOLOv8 ONNX model is configured".into());
        }
        Ok(Self {
            models,
            threshold: config.confidence_threshold,
        })
    }

    pub fn models(&self) -> Vec<&'static str> {
        self.models.iter().map(|model| model.kind.name()).collect()
    }

    pub fn detect(&mut self, bytes: &[u8]) -> Result<Vec<DetectionItem>, DetectionError> {
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| DetectionError::InvalidImage)?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(80 * 1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|_| DetectionError::InvalidImage)?
            .into_rgb8();
        let mut detections = Vec::new();
        for model in &mut self.models {
            detections.extend(
                model
                    .predict(&image, self.threshold)
                    .map_err(DetectionError::Inference)?,
            );
        }
        Ok(detections)
    }
}

fn letterbox(image: &RgbImage, width: usize, height: usize) -> Vec<f32> {
    let scale = (width as f32 / image.width() as f32).min(height as f32 / image.height() as f32);
    let resized_w = ((image.width() as f32 * scale).round() as usize).clamp(1, width);
    let resized_h = ((image.height() as f32 * scale).round() as usize).clamp(1, height);
    let resized = image::imageops::resize(
        image,
        resized_w as u32,
        resized_h as u32,
        FilterType::Triangle,
    );
    let left = (width - resized_w) / 2;
    let top = (height - resized_h) / 2;
    let plane = width * height;
    let mut input = vec![114.0 / 255.0; 3 * plane];
    for (x, y, pixel) in resized.enumerate_pixels() {
        let index = (y as usize + top) * width + x as usize + left;
        for (channel, value) in pixel.0.into_iter().enumerate() {
            input[channel * plane + index] = f32::from(value) / 255.0;
        }
    }
    input
}

struct Candidate {
    label: &'static str,
    score: f32,
    bounds: [f32; 4],
}

fn decode_predictions(
    shape: &[i64],
    values: &[f32],
    kind: ModelKind,
    threshold: f32,
) -> Result<Vec<DetectionItem>, String> {
    let fields = kind.classes() + 4;
    let [1, first, second] = shape else {
        return Err("Expected one batch of YOLOv8 detections".into());
    };
    let (first, second) = (*first as usize, *second as usize);
    let channels_first = first == fields;
    let count = if channels_first { second } else { first };
    if (!channels_first && second != fields) || count.checked_mul(fields) != Some(values.len()) {
        return Err("Unexpected YOLOv8 output dimensions".into());
    }

    let mut candidates = Vec::new();
    for index in 0..count {
        let get = |field| {
            let offset = if channels_first {
                field * count + index
            } else {
                index * fields + field
            };
            values[offset]
        };
        let (class_id, score) = (0..kind.classes())
            .map(|id| (id, get(4 + id)))
            .filter(|(_, score)| score.is_finite() && *score <= 1.0 && *score >= threshold)
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap_or((0, -1.0));
        if score < threshold {
            continue;
        }
        let Some(label) = kind.label(class_id) else {
            continue;
        };
        let [x, y, w, h] = [get(0), get(1), get(2), get(3)];
        if [x, y, w, h].iter().all(|v| v.is_finite()) && w > 0.0 && h > 0.0 {
            candidates.push(Candidate {
                label,
                score,
                bounds: [x - w / 2.0, y - h / 2.0, x + w / 2.0, y + h / 2.0],
            });
        }
    }
    candidates.sort_unstable_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Candidate> = Vec::new();
    for item in candidates {
        if kept
            .iter()
            .any(|prior| prior.label == item.label && iou(prior.bounds, item.bounds) > 0.5)
        {
            continue;
        }
        kept.push(item);
        if kept.len() == 30 {
            break;
        }
    }
    Ok(kept
        .into_iter()
        .map(|item| DetectionItem {
            object_type: item.label.into(),
            event_type: if matches!(item.label, "person" | "fire" | "smoke") {
                item.label
            } else {
                "object"
            }
            .into(),
            confidence: f64::from(item.score),
        })
        .collect())
}

fn iou(a: [f32; 4], b: [f32; 4]) -> f32 {
    let width = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let height = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let intersection = width * height;
    let area = |r: [f32; 4]| (r[2] - r[0]) * (r[3] - r[1]);
    intersection / (area(a) + area(b) - intersection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppresses_duplicate_fire_boxes_and_preserves_smoke() {
        let rows = [
            [100.0, 100.0, 80.0, 80.0, 0.90, 0.05],
            [103.0, 103.0, 80.0, 80.0, 0.85, 0.05],
            [100.0, 100.0, 80.0, 80.0, 0.05, 0.75],
            [200.0, 200.0, 80.0, 80.0, 0.30, 0.01],
        ];
        let channels: Vec<_> = (0..6)
            .flat_map(|field| rows.iter().map(move |row| row[field]))
            .collect();
        let detections =
            decode_predictions(&[1, 6, 4], &channels, ModelKind::FireSmoke, 0.5).unwrap();

        let events: Vec<_> = detections
            .iter()
            .map(|item| item.event_type.as_str())
            .collect();
        assert_eq!(events, ["fire", "smoke"]);
    }
}
