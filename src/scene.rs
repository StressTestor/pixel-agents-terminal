// Scene compositor

use image::{Rgba, RgbaImage};
use crate::agent::AgentState;
use crate::grid::{Grid, Pos, GRID_WIDTH, GRID_HEIGHT, TILE_SIZE, Tile};
use crate::sprites::{generate_sprite, SPRITE_WIDTH, SPRITE_HEIGHT};

pub struct AgentView {
    pub pos: Pos,
    pub hue: f32,
    pub state: AgentState,
    pub frame: u32,
    pub direction: u8,
}

/// Composite the full office scene: tiles first, then agents z-sorted by y.
pub fn composite_scene(grid: &Grid, agents: &[AgentView]) -> RgbaImage {
    let width = GRID_WIDTH as u32 * TILE_SIZE;
    let height = GRID_HEIGHT as u32 * TILE_SIZE;
    let mut img = RgbaImage::new(width, height);

    // Draw all tiles (background layer)
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let pos = Pos { x, y };
            let tile = grid.tiles[y][x];
            draw_tile(&mut img, pos, tile);
        }
    }

    // Sort agents by y-position (painter's algorithm: lower y first)
    let mut sorted: Vec<&AgentView> = agents.iter().collect();
    sorted.sort_by_key(|a| a.pos.y);

    // Draw agents on top
    for agent in sorted {
        let sprite = generate_sprite(agent.hue, agent.state, agent.frame, agent.direction);
        blit_sprite(&mut img, &sprite, agent.pos);
    }

    img
}

/// Fill a TILE_SIZE x TILE_SIZE rectangle at `pos` with the tile's color.
fn draw_tile(img: &mut RgbaImage, pos: Pos, tile: Tile) {
    let color = match tile {
        Tile::Floor => Rgba([60, 60, 70, 255]),
        Tile::Wall => Rgba([100, 100, 110, 255]),
        Tile::Desk(_) => Rgba([139, 90, 43, 255]),
        Tile::Chair(_) => Rgba([80, 80, 90, 255]),
        Tile::WaterCooler => Rgba([100, 150, 255, 255]),
    };

    let px = pos.x as u32 * TILE_SIZE;
    let py = pos.y as u32 * TILE_SIZE;

    for dy in 0..TILE_SIZE {
        for dx in 0..TILE_SIZE {
            let ix = px + dx;
            let iy = py + dy;
            if ix < img.width() && iy < img.height() {
                img.put_pixel(ix, iy, color);
            }
        }
    }
}

/// Draw a sprite at grid position `pos`.
/// - Centered horizontally within the tile
/// - Bottom-aligned (sprite hangs down from tile bottom edge, taller sprites extend upward)
/// - Transparent pixels (alpha == 0) are skipped
fn blit_sprite(img: &mut RgbaImage, sprite: &RgbaImage, pos: Pos) {
    let tile_px = pos.x as u32 * TILE_SIZE;
    let tile_py = pos.y as u32 * TILE_SIZE;

    // Center horizontally: offset from tile left edge
    let h_offset = ((TILE_SIZE as i32) - (SPRITE_WIDTH as i32)) / 2;
    // Bottom-align: sprite bottom == tile bottom
    // tile bottom is at tile_py + TILE_SIZE
    // sprite bottom (row SPRITE_HEIGHT-1) should land at tile_py + TILE_SIZE - 1
    let v_offset = tile_py as i32 + TILE_SIZE as i32 - SPRITE_HEIGHT as i32;

    for sy in 0..SPRITE_HEIGHT {
        for sx in 0..SPRITE_WIDTH {
            let pixel = sprite.get_pixel(sx, sy);
            if pixel[3] == 0 {
                continue;
            }

            let ix = tile_px as i32 + h_offset + sx as i32;
            let iy = v_offset + sy as i32;

            if ix >= 0 && iy >= 0 && (ix as u32) < img.width() && (iy as u32) < img.height() {
                img.put_pixel(ix as u32, iy as u32, *pixel);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    // 1. Empty office produces an image with correct dimensions (256x192)
    #[test]
    fn test_scene_dimensions() {
        let grid = Grid::default_office();
        let img = composite_scene(&grid, &[]);
        assert_eq!(img.width(), GRID_WIDTH as u32 * TILE_SIZE);
        assert_eq!(img.height(), GRID_HEIGHT as u32 * TILE_SIZE);
        assert_eq!(img.width(), 256);
        assert_eq!(img.height(), 192);
    }

    // 2. Empty office has non-transparent pixels (walls and floor are drawn)
    #[test]
    fn test_empty_scene_not_blank() {
        let grid = Grid::default_office();
        let img = composite_scene(&grid, &[]);
        // Every tile is opaque, so every pixel should be fully opaque
        let has_opaque = img.pixels().any(|p| p[3] == 255);
        assert!(has_opaque, "scene should have opaque pixels from tile rendering");
    }

    // 3. Scene with an agent differs from scene without
    #[test]
    fn test_agent_appears_in_scene() {
        let grid = Grid::default_office();

        let without_agent = composite_scene(&grid, &[]);

        let agent = AgentView {
            pos: Pos { x: 5, y: 5 },
            hue: 120.0,
            state: AgentState::Idle,
            frame: 0,
            direction: 0,
        };
        let with_agent = composite_scene(&grid, &[agent]);

        let differs = without_agent
            .enumerate_pixels()
            .any(|(x, y, p)| p != with_agent.get_pixel(x, y));

        assert!(differs, "scene with agent should differ from scene without agent");
    }
}
