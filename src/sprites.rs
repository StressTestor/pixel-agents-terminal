// Procedural sprite generation

use image::{Rgba, RgbaImage};
use crate::agent::AgentState;

pub const SPRITE_WIDTH: u32 = 12;
pub const SPRITE_HEIGHT: u32 = 16;

/// Standard HSL to RGBA conversion.
/// h: 0-360, s: 0-1, l: 0-1
pub fn hsl_to_rgba(h: f32, s: f32, l: f32) -> Rgba<u8> {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());

    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let m = l - c / 2.0;
    Rgba([
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
        255,
    ])
}

/// Generate a 12x16 procedural sprite for an agent.
///
/// Layout (y=0 is top):
///   y=0        : state overlay row
///   y=1..4     : head (narrower rectangle)
///   y=5..15    : body (full-width minus 2px padding each side)
///
/// Walking animation: frame 1 bobs the body+head up by 1px.
/// Direction controls eye position:
///   0=down, 1=up, 2=right, 3=left
pub fn generate_sprite(hue: f32, state: AgentState, frame: u32, direction: u8) -> RgbaImage {
    let mut img = RgbaImage::new(SPRITE_WIDTH, SPRITE_HEIGHT);

    // transparent background
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 0]);
    }

    let body_color = hsl_to_rgba(hue, 0.7, 0.5);
    // head is slightly lighter
    let head_color = hsl_to_rgba(hue, 0.6, 0.65);
    let eye_color = Rgba([255, 255, 255, 255]);

    // vertical bob: frame 1 shifts body+head up by 1
    let bob: i32 = if frame == 1 { -1 } else { 0 };

    // --- body: x=2..10, y=7..15 (with bob) ---
    let body_x_start: u32 = 2;
    let body_x_end: u32 = 10; // exclusive
    let body_y_start: i32 = 7 + bob;
    let body_y_end: i32 = 16; // exclusive (clamped)

    for y in body_y_start..body_y_end {
        if y < 0 || y >= SPRITE_HEIGHT as i32 {
            continue;
        }
        for x in body_x_start..body_x_end {
            img.put_pixel(x, y as u32, body_color);
        }
    }

    // --- head: x=3..9, y=2..7 (with bob) ---
    let head_x_start: u32 = 3;
    let head_x_end: u32 = 9; // exclusive
    let head_y_start: i32 = 2 + bob;
    let head_y_end: i32 = 7 + bob; // exclusive

    for y in head_y_start..head_y_end {
        if y < 0 || y >= SPRITE_HEIGHT as i32 {
            continue;
        }
        for x in head_x_start..head_x_end {
            img.put_pixel(x, y as u32, head_color);
        }
    }

    // --- eye: 2px dot based on direction, positioned within head ---
    // Eye center relative to head center (x=6, y=head_y_start+2)
    let head_center_x: i32 = 6;
    let head_center_y: i32 = head_y_start + 2;

    let (eye_dx, eye_dy): (i32, i32) = match direction {
        0 => (0, 1),  // down
        1 => (0, -1), // up
        2 => (1, 0),  // right
        3 => (-1, 0), // left
        _ => (0, 0),
    };

    let eye_x = head_center_x + eye_dx;
    let eye_y = head_center_y + eye_dy;

    // draw 2x2 eye dot
    for dy in 0..2i32 {
        for dx in 0..2i32 {
            let px = eye_x + dx;
            let py = eye_y + dy;
            if px >= 0 && px < SPRITE_WIDTH as i32 && py >= 0 && py < SPRITE_HEIGHT as i32 {
                img.put_pixel(px as u32, py as u32, eye_color);
            }
        }
    }

    // --- state overlays at y=0 (above head) ---
    match state {
        AgentState::Typing => {
            // yellow cursor dots at (5,0) and (6,0)
            let yellow = Rgba([255, 220, 0, 255]);
            img.put_pixel(5, 0, yellow);
            img.put_pixel(6, 0, yellow);
        }
        AgentState::Reading => {
            // light blue doc bar at x=4..8, y=0
            let light_blue = Rgba([100, 180, 255, 255]);
            for x in 4..8u32 {
                img.put_pixel(x, 0, light_blue);
            }
        }
        AgentState::Waiting => {
            // white "..." dots at (3,0), (5,0), (7,0)
            let white = Rgba([255, 255, 255, 255]);
            img.put_pixel(3, 0, white);
            img.put_pixel(5, 0, white);
            img.put_pixel(7, 0, white);
        }
        _ => {}
    }

    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentState;

    // 1. sprite is always 12x16
    #[test]
    fn test_sprite_dimensions() {
        let img = generate_sprite(180.0, AgentState::Idle, 0, 0);
        assert_eq!(img.width(), SPRITE_WIDTH);
        assert_eq!(img.height(), SPRITE_HEIGHT);
    }

    // 2. different hues produce different colors at body center pixel
    #[test]
    fn test_different_hues_produce_different_colors() {
        // body center: x=6, y=11 (body runs y=7..16)
        let img_red = generate_sprite(0.0, AgentState::Idle, 0, 0);
        let img_green = generate_sprite(120.0, AgentState::Idle, 0, 0);
        let red_pixel = img_red.get_pixel(6, 11);
        let green_pixel = img_green.get_pixel(6, 11);
        assert_ne!(red_pixel, green_pixel, "hue 0 and hue 120 should produce different body colors");
    }

    // 3. walking frames differ (frame 0 vs frame 1 have pixel differences)
    #[test]
    fn test_walking_frames_differ() {
        let frame0 = generate_sprite(200.0, AgentState::Walking, 0, 0);
        let frame1 = generate_sprite(200.0, AgentState::Walking, 1, 0);
        let differs = frame0
            .enumerate_pixels()
            .any(|(x, y, p)| p != frame1.get_pixel(x, y));
        assert!(differs, "frame 0 and frame 1 should differ due to vertical bob");
    }

    // 4. h=0, s=1, l=0.5 -> pure red (255, 0, 0, 255)
    #[test]
    fn test_hsl_red() {
        let c = hsl_to_rgba(0.0, 1.0, 0.5);
        assert_eq!(c[0], 255, "R should be 255");
        assert_eq!(c[1], 0,   "G should be 0");
        assert_eq!(c[2], 0,   "B should be 0");
        assert_eq!(c[3], 255, "A should be 255");
    }

    // 5. h=120, s=1, l=0.5 -> pure green (0, 255, 0, 255)
    #[test]
    fn test_hsl_green() {
        let c = hsl_to_rgba(120.0, 1.0, 0.5);
        assert_eq!(c[0], 0,   "R should be 0");
        assert_eq!(c[1], 255, "G should be 255");
        assert_eq!(c[2], 0,   "B should be 0");
        assert_eq!(c[3], 255, "A should be 255");
    }
}
