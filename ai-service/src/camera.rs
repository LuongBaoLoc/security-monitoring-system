use std::io::{Cursor, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use image::{codecs::jpeg::JpegEncoder, DynamicImage, Rgb, RgbImage};
use tracing::warn;
use url::Url;

pub const DEFAULT_FFMPEG_TIMEOUT: Duration = Duration::from_secs(10);

/// Ẩn thông tin đăng nhập (user/password), query và fragment trong RTSP URL trước khi log.
pub fn redact_rtsp_url(source: &str) -> String {
    let trimmed = source.trim();
    if let Ok(mut parsed) = Url::parse(trimmed) {
        if !parsed.username().is_empty() || parsed.password().is_some() {
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
        }
        parsed.set_query(None);
        parsed.set_fragment(None);
        return parsed.to_string();
    }

    if let Some(scheme_pos) = trimmed.find("://") {
        let rest = &trimmed[scheme_pos + 3..];
        let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let auth = &rest[..auth_end];
        if let Some(at_pos) = auth.rfind('@') {
            let host = &auth[at_pos + 1..];
            let path = rest[auth_end..].split(['?', '#']).next().unwrap_or("");
            return format!("{}://{}{}", &trimmed[..scheme_pos], host, path);
        }
    }

    trimmed.to_string()
}

/// Xóa mọi thông tin đăng nhập RTSP trong chuỗi stderr do `ffmpeg` trả về.
pub fn sanitize_rtsp_log(raw_text: &str, raw_source: &str) -> String {
    let masked = redact_rtsp_url(raw_source);
    let mut text = if !raw_source.is_empty() {
        raw_text.replace(raw_source, &masked)
    } else {
        raw_text.to_string()
    };

    for prefix in ["rtsp://", "rtsps://"] {
        let mut cursor = 0usize;
        while let Some(rel) = text[cursor..].find(prefix) {
            let start = cursor + rel + prefix.len();
            let tail = &text[start..];
            let end_rel = tail
                .find(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | ',' | ')' | ']'))
                .unwrap_or(tail.len());
            let segment = &tail[..end_rel];
            if let Some(at_pos) = segment.rfind('@') {
                text.replace_range(start..start + at_pos + 1, "");
                cursor = start;
            } else {
                cursor = start + end_rel;
            }
        }
    }

    text.trim().to_string()
}

pub fn capture_frame_bytes(source: &str, jpeg_quality: u8) -> Result<Vec<u8>, String> {
    capture_frame_bytes_with_timeout(source, jpeg_quality, DEFAULT_FFMPEG_TIMEOUT)
}

/// Lấy 1 khung hình JPEG từ file ảnh hoặc luồng RTSP bằng `ffmpeg`, kèm giới hạn thời gian chờ
/// và tự động ẩn user/password trong mọi thông báo lỗi.
pub fn capture_frame_bytes_with_timeout(
    source: &str,
    jpeg_quality: u8,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let safe_source = redact_rtsp_url(source);

    if Path::new(source).is_file() {
        let bytes = std::fs::read(source)
            .map_err(|e| format!("Unable to read file {safe_source}: {e}"))?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("Unable to decode image {safe_source}: {e}"))?;
        return encode_jpeg(&img, jpeg_quality);
    }

    let limit = if timeout.is_zero() {
        DEFAULT_FFMPEG_TIMEOUT
    } else {
        timeout
    };
    let rw_timeout_us = limit.as_micros().min(u64::MAX as u128).to_string();

    let mut child = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-loglevel",
            "error",
            "-rtsp_transport",
            "tcp",
            "-rw_timeout",
            &rw_timeout_us,
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
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            let msg = format!("Unable to spawn ffmpeg for {safe_source}: {e}");
            warn!("{msg}");
            msg
        })?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let out_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stdout_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stderr_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + limit;
    let exit_opt = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let msg = format!("Failed waiting for ffmpeg ({safe_source}): {e}");
                warn!("{msg}");
                return Err(msg);
            }
        }
    };

    let stdout = out_reader.join().unwrap_or_default();
    let stderr_raw = String::from_utf8_lossy(&err_reader.join().unwrap_or_default()).to_string();
    let stderr_safe = sanitize_rtsp_log(&stderr_raw, source);

    let Some(status) = exit_opt else {
        let msg = format!(
            "Camera capture timed out after {}s for {safe_source}",
            limit.as_secs()
        );
        warn!("{msg}");
        return Err(msg);
    };

    if !status.success() || stdout.is_empty() {
        let reason = if stderr_safe.is_empty() {
            format!("exit status {:?}", status.code())
        } else {
            stderr_safe
        };
        let msg = format!("Failed to read frame from {safe_source}: {reason}");
        warn!("{msg}");
        return Err(msg);
    }

    Ok(stdout)
}

/// Tạo ảnh mẫu cảnh cháy & khói trong nhà kho (640x480) để kiểm thử trọn luồng PCCC.
pub fn create_sample_test_image(width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut img = RgbImage::from_pixel(width, height, Rgb([30, 38, 52]));

    // Nền bê tông nhà kho và giá kệ hàng hóa
    let ground_y = ((height as f64) * 0.68) as u32;
    fill_rect(&mut img, 0, ground_y, width, height, Rgb([55, 62, 74]));
    fill_rect(&mut img, 40, 90, 210, ground_y, Rgb([72, 84, 100]));
    fill_rect(&mut img, 450, 110, 600, ground_y, Rgb([68, 78, 92]));

    // Đám khói xám bốc lên trên khu vực cháy (smoke)
    fill_ellipse(&mut img, 320, 185, 78, 52, Rgb([130, 138, 148]));
    fill_ellipse(&mut img, 345, 135, 64, 42, Rgb([152, 160, 170]));
    fill_ellipse(&mut img, 290, 150, 52, 36, Rgb([118, 126, 136]));

    // Ngọn lửa đỏ cam sáng rực tại tâm sự cố (fire)
    fill_ellipse(&mut img, 320, 318, 48, 56, Rgb([235, 68, 32]));
    fill_ellipse(&mut img, 320, 310, 32, 42, Rgb([250, 145, 25]));
    fill_ellipse(&mut img, 320, 325, 18, 24, Rgb([255, 230, 95]));

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_rtsp_user_password_and_query() {
        let raw = "rtsp://admin:MySecret99@192.168.1.21:554/stream1?token=xyz";
        let masked = redact_rtsp_url(raw);
        assert_eq!(masked, "rtsp://192.168.1.21:554/stream1");
        assert!(!masked.contains("admin"));
        assert!(!masked.contains("MySecret99"));
        assert!(!masked.contains("token"));
    }

    #[test]
    fn strips_credentials_from_ffmpeg_stderr() {
        let raw = "rtsp://op:p@ss123@10.10.0.5:554/live";
        let err = "rtsp://op:p@ss123@10.10.0.5:554/live: Connection timed out";
        let safe = sanitize_rtsp_log(err, raw);
        assert!(!safe.contains("p@ss123"));
        assert!(!safe.contains("op:"));
        assert!(safe.contains("10.10.0.5:554/live"));
    }
}
