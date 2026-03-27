// Kitty graphics protocol renderer

use image::{ImageBuffer, Rgba};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

const CHUNK_SIZE: usize = 4096;

pub fn chunk_kitty_payload(header: &str, base64_data: &[u8]) -> Vec<Vec<u8>> {
    if base64_data.is_empty() {
        let mut seq = Vec::new();
        seq.extend_from_slice(b"\x1b_G");
        seq.extend_from_slice(header.as_bytes());
        seq.extend_from_slice(b",m=0;");
        seq.extend_from_slice(b"\x1b\\");
        return vec![seq];
    }

    let windows: Vec<&[u8]> = base64_data.chunks(CHUNK_SIZE).collect();
    let num_chunks = windows.len();
    let mut chunks: Vec<Vec<u8>> = Vec::with_capacity(num_chunks);

    for (i, window) in windows.iter().enumerate() {
        let is_last = i == num_chunks - 1;
        let m_flag = if is_last { b"m=0;" as &[u8] } else { b"m=1;" };

        let mut seq = Vec::new();
        seq.extend_from_slice(b"\x1b_G");

        if i == 0 {
            seq.extend_from_slice(header.as_bytes());
            seq.push(b',');
        }

        seq.extend_from_slice(m_flag);
        seq.extend_from_slice(window);
        seq.extend_from_slice(b"\x1b\\");

        chunks.push(seq);
    }

    chunks
}

pub fn render_frame(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, scale: u32) -> Vec<u8> {
    let scale = scale.max(1);

    // Scale the image if needed (nearest-neighbor preserves pixel art crispness)
    let (raw_rgba, width, height) = if scale > 1 {
        let scaled = image::imageops::resize(
            img,
            img.width() * scale,
            img.height() * scale,
            image::imageops::FilterType::Nearest,
        );
        let w = scaled.width();
        let h = scaled.height();
        (scaled.into_raw(), w, h)
    } else {
        (img.as_raw().clone(), img.width(), img.height())
    };

    let b64 = STANDARD.encode(&raw_rgba);
    let b64_bytes = b64.as_bytes();

    let header = format!("a=T,f=32,s={},v={},q=2,C=1", width, height);

    let mut out: Vec<u8> = Vec::new();

    // DECSC - save cursor position
    out.extend_from_slice(b"\x1b7");

    // move cursor to top-left origin
    out.extend_from_slice(b"\x1b[1;1H");

    // delete all images (avoids Ghostty image ID replacement bug #6711)
    out.extend_from_slice(b"\x1b_Ga=d,d=a,q=2;\x1b\\");

    // transmit + display via chunked Kitty protocol (raw RGBA, f=32)
    let chunks = chunk_kitty_payload(&header, b64_bytes);
    for chunk in chunks {
        out.extend_from_slice(&chunk);
    }

    // DECRC - restore cursor position
    out.extend_from_slice(b"\x1b8");

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_payload() {
        let chunks = chunk_kitty_payload("a=T,f=32,s=1,v=1,q=2,C=1", b"");
        assert_eq!(chunks.len(), 1);
        let chunk = String::from_utf8(chunks[0].clone()).unwrap();
        assert!(chunk.starts_with("\x1b_G"));
        assert!(chunk.contains("m=0"));
        assert!(chunk.ends_with("\x1b\\"));
    }

    #[test]
    fn test_small_payload() {
        let data = b"AQAAAA==";
        let chunks = chunk_kitty_payload("a=T,f=32,s=1,v=1,q=2,C=1", data);
        assert_eq!(chunks.len(), 1);
        let chunk = String::from_utf8(chunks[0].clone()).unwrap();
        assert!(chunk.contains("m=0"));
        assert!(chunk.contains("AQAAAA=="));
    }

    #[test]
    fn test_exact_chunk_boundary() {
        let data = vec![b'A'; 4096];
        let chunks = chunk_kitty_payload("a=T,f=32,s=1,v=1,q=2,C=1", &data);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_one_byte_over_boundary() {
        let data = vec![b'A'; 4097];
        let chunks = chunk_kitty_payload("a=T,f=32,s=1,v=1,q=2,C=1", &data);
        assert_eq!(chunks.len(), 2);
        let first = String::from_utf8(chunks[0].clone()).unwrap();
        assert!(first.contains("a=T"));
        assert!(first.contains("m=1"));
        let last = String::from_utf8(chunks[1].clone()).unwrap();
        assert!(last.contains("m=0"));
        assert!(!last.contains("a=T"));
    }

    #[test]
    fn test_three_chunks() {
        let data = vec![b'A'; 4096 * 2 + 100];
        let chunks = chunk_kitty_payload("a=T,f=32,s=1,v=1,q=2,C=1", &data);
        assert_eq!(chunks.len(), 3);
        let c0 = String::from_utf8(chunks[0].clone()).unwrap();
        let c1 = String::from_utf8(chunks[1].clone()).unwrap();
        let c2 = String::from_utf8(chunks[2].clone()).unwrap();
        assert!(c0.contains("m=1"));
        assert!(c1.contains("m=1"));
        assert!(c2.contains("m=0"));
    }

    #[test]
    fn test_render_frame_structure() {
        use image::{ImageBuffer, Rgba};
        let img = ImageBuffer::from_fn(2, 2, |_x, _y| Rgba([255u8, 0, 0, 255]));
        let output = render_frame(&img, 1);
        let out_str = String::from_utf8_lossy(&output);
        assert!(out_str.starts_with("\x1b7"));      // DECSC
        assert!(out_str.contains("\x1b[1;1H"));     // cursor to origin
        assert!(out_str.contains("a=d,d=a,q=2"));  // delete all
        assert!(out_str.contains("a=T,f=32"));      // transmit RGBA
        assert!(out_str.contains("s=2,v=2"));       // dimensions
        assert!(out_str.ends_with("\x1b8"));         // DECRC
    }

    #[test]
    fn test_render_frame_scaled() {
        use image::{ImageBuffer, Rgba};
        let img = ImageBuffer::from_fn(2, 2, |_x, _y| Rgba([255u8, 0, 0, 255]));
        let output = render_frame(&img, 3);
        let out_str = String::from_utf8_lossy(&output);
        assert!(out_str.contains("s=6,v=6")); // 2*3=6
    }
}
