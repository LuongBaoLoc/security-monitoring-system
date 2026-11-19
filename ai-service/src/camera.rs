use std::io::Cursor;
use std::path::Path;
use std::process::Command;
use image::{codecs::jpeg::JpegEncoder, DynamicImage, Rgb, RgbImage};

pub fn capture_frame_bytes(source: &str, jpeg_quality: u8) -> Result<Vec<u8>, String> {
    if Path::new(source).is_file() {
        let bytes = std::fs::read(source)
            .map_err(|e| format!("Unable to read file {source}: {e}"))?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("Unable to decode image {source}: {e}"))?;
        return encode_jpeg(&img, jpeg_quality);
    }

    // Attempt to grab 1 frame from RTSP/video stream via ffmpeg CLI
    let output = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-rtsp_transport",
            "tcp",
            "-i",
            source,
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .map_err(|e| format!("Unable to open camera stream {source}: {e}"))?;

    if !output.status.success() || output.stdout.is_empty() {
        return Err(format!("Failed to read frame from camera stream: {source}"));
    }

    Ok(output.stdout)
}

pub fn create_sample_test_image(width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut img = RgbImage::from_pixel(width, height, Rgb([215, 226, 236]));

    let ground_y = ((height as f64) * 0.65) as u32;
    fill_rect(&mut img, 0, ground_y, width, height, Rgb([160, 174, 162]));
    fill_rect(&mut img, 40, 80, 260, ground_y, Rgb([120, 138, 156]));

    // Person-like silhouette for pipeline testing
    fill_ellipse(&mut img, 320, 170, 20, 20, Rgb([45, 55, 72]));
    fill_rect(&mut img, 295, 195, 345, 300, Rgb([30, 64, 175]));
    fill_rect(&mut img, 298, 300, 318, 380, Rgb([30, 41, 59]));
    fill_rect(&mut img, 322, 300, 342, 380, Rgb([30, 41, 59]));

    let dyn_img = DynamicImage::ImageRgb8(img);
    encode_jpeg(&dyn_img, 90)
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .encode_image(img)
        .map_err(|e| format!("Failed to encode frame as JPEG: {e}"))?;
    Ok(buf.into_inner())
}

fn fill_rect(img: &mut RgbImage, x0: u32, y0: u32, x1: u32, y1: u32, color: Rgb<u8>) {
    let max_x = x1.min(img.width());
    let max_y = y1.min(img.height());
    for y in y0.min(max_y)..max_y {
        for x in x0.min(max_x)..max_x {
            img.put_pixel(x, y, color);
        }
    }
}

fn fill_ellipse(img: &mut RgbImage, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgb<u8>) {
    if rx <= 0 || ry <= 0 {
        return;
    }
    let rx2 = (rx * rx) as f64;
    let ry2 = (ry * ry) as f64;
    for y in (cy - ry).max(0)..=(cy + ry).min(img.height() as i32 - 1) {
        for x in (cx - rx).max(0)..=(cx + rx).min(img.width() as i32 - 1) {
            let dx = (x - cx) as f64;
            let dy = (y - cy) as f64;
            if (dx * dx) / rx2 + (dy * dy) / ry2 <= 1.0 {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}
