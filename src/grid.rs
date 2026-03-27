// Office grid + BFS pathfinding

use std::collections::VecDeque;

pub const GRID_WIDTH: usize = 16;
pub const GRID_HEIGHT: usize = 12;
pub const TILE_SIZE: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Floor,
    Wall,
    Desk(usize),
    Chair(usize),
    WaterCooler,
}

pub struct Grid {
    pub tiles: [[Tile; GRID_WIDTH]; GRID_HEIGHT],
    pub desk_positions: Vec<Pos>,
    pub overflow_ring: Vec<Pos>,
}

impl Grid {
    pub fn default_office() -> Self {
        let mut tiles = [[Tile::Floor; GRID_WIDTH]; GRID_HEIGHT];

        // perimeter walls
        for x in 0..GRID_WIDTH {
            tiles[0][x] = Tile::Wall;
            tiles[GRID_HEIGHT - 1][x] = Tile::Wall;
        }
        for y in 0..GRID_HEIGHT {
            tiles[y][0] = Tile::Wall;
            tiles[y][GRID_WIDTH - 1] = Tile::Wall;
        }

        // 6 desks: 2 rows of 3, at x=3,7,11 y=3,7
        // chairs directly below each desk at y+1
        let desk_xs = [3usize, 7, 11];
        let desk_ys = [3usize, 7];
        let mut desk_id = 0;
        let mut desk_positions = Vec::new();

        for &dy in &desk_ys {
            for &dx in &desk_xs {
                tiles[dy][dx] = Tile::Desk(desk_id);
                tiles[dy + 1][dx] = Tile::Chair(desk_id);
                desk_positions.push(Pos { x: dx, y: dy + 1 });
                desk_id += 1;
            }
        }

        // water cooler at (13, 5) — x=13, y=5
        tiles[5][13] = Tile::WaterCooler;

        // overflow ring: 8 tiles surrounding (13,5)
        // filter to walkable (non-wall, non-desk) tiles
        let cooler_x = 13i32;
        let cooler_y = 5i32;
        let mut overflow_ring = Vec::new();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = cooler_x + dx;
                let ny = cooler_y + dy;
                if nx >= 0
                    && ny >= 0
                    && (nx as usize) < GRID_WIDTH
                    && (ny as usize) < GRID_HEIGHT
                {
                    let pos = Pos {
                        x: nx as usize,
                        y: ny as usize,
                    };
                    let tile = tiles[ny as usize][nx as usize];
                    if matches!(tile, Tile::Floor | Tile::Chair(_)) {
                        overflow_ring.push(pos);
                    }
                }
            }
        }

        Grid {
            tiles,
            desk_positions,
            overflow_ring,
        }
    }

    pub fn is_walkable(&self, pos: Pos) -> bool {
        if pos.x >= GRID_WIDTH || pos.y >= GRID_HEIGHT {
            return false;
        }
        matches!(
            self.tiles[pos.y][pos.x],
            Tile::Floor | Tile::Chair(_) | Tile::WaterCooler
        )
    }
}

pub fn pathfind(grid: &Grid, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    if from == to {
        return Some(Vec::new());
    }

    // BFS
    let mut visited = vec![vec![false; GRID_WIDTH]; GRID_HEIGHT];
    let mut came_from: Vec<Vec<Option<Pos>>> = vec![vec![None; GRID_WIDTH]; GRID_HEIGHT];
    let mut queue = VecDeque::new();

    visited[from.y][from.x] = true;
    queue.push_back(from);

    let dirs: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

    while let Some(current) = queue.pop_front() {
        if current == to {
            // reconstruct path
            let mut path = Vec::new();
            let mut pos = to;
            while pos != from {
                path.push(pos);
                pos = came_from[pos.y][pos.x].unwrap();
            }
            path.reverse();
            return Some(path);
        }

        for (dx, dy) in &dirs {
            let nx = current.x as i32 + dx;
            let ny = current.y as i32 + dy;
            if nx < 0 || ny < 0 || nx as usize >= GRID_WIDTH || ny as usize >= GRID_HEIGHT {
                continue;
            }
            let next = Pos {
                x: nx as usize,
                y: ny as usize,
            };
            if visited[next.y][next.x] {
                continue;
            }
            // walkable OR destination is a Chair (agents walk to sit down)
            let tile = grid.tiles[next.y][next.x];
            let passable = matches!(tile, Tile::Floor | Tile::Chair(_) | Tile::WaterCooler)
                || (next == to && matches!(tile, Tile::Chair(_)));
            if !passable {
                continue;
            }
            visited[next.y][next.x] = true;
            came_from[next.y][next.x] = Some(current);
            queue.push_back(next);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_office_has_six_desks() {
        let grid = Grid::default_office();
        assert_eq!(grid.desk_positions.len(), 6);

        // all desk_positions should have Chair tiles
        for pos in &grid.desk_positions {
            assert!(
                matches!(grid.tiles[pos.y][pos.x], Tile::Chair(_)),
                "expected Chair at {:?}, got {:?}",
                pos,
                grid.tiles[pos.y][pos.x]
            );
        }
    }

    #[test]
    fn test_default_office_has_walls() {
        let grid = Grid::default_office();

        // corners
        assert_eq!(grid.tiles[0][0], Tile::Wall);
        assert_eq!(grid.tiles[0][GRID_WIDTH - 1], Tile::Wall);
        assert_eq!(grid.tiles[GRID_HEIGHT - 1][0], Tile::Wall);
        assert_eq!(grid.tiles[GRID_HEIGHT - 1][GRID_WIDTH - 1], Tile::Wall);

        // top and bottom edges
        for x in 0..GRID_WIDTH {
            assert_eq!(grid.tiles[0][x], Tile::Wall);
            assert_eq!(grid.tiles[GRID_HEIGHT - 1][x], Tile::Wall);
        }

        // left and right edges
        for y in 0..GRID_HEIGHT {
            assert_eq!(grid.tiles[y][0], Tile::Wall);
            assert_eq!(grid.tiles[y][GRID_WIDTH - 1], Tile::Wall);
        }
    }

    #[test]
    fn test_default_office_has_floor_interior() {
        let grid = Grid::default_office();
        assert_eq!(grid.tiles[1][1], Tile::Floor);
        assert_eq!(grid.tiles[5][5], Tile::Floor);
    }

    #[test]
    fn test_pathfind_straight_line() {
        let grid = Grid::default_office();
        let from = Pos { x: 1, y: 1 };
        let to = Pos { x: 4, y: 1 };
        let path = pathfind(&grid, from, to).expect("should find path");

        // path should not include start, should include destination
        assert!(!path.is_empty());
        assert_eq!(*path.last().unwrap(), to);
        assert!(!path.contains(&from));

        // length should be 3 steps (1,1)->(2,1)->(3,1)->(4,1)
        assert_eq!(path.len(), 3);
    }

    #[test]
    fn test_pathfind_same_position() {
        let grid = Grid::default_office();
        let pos = Pos { x: 5, y: 5 };
        let path = pathfind(&grid, pos, pos).expect("same position should return Some");
        assert!(path.is_empty());
    }

    #[test]
    fn test_pathfind_around_obstacle() {
        // build a grid with a horizontal wall blocking direct path, forcing a detour
        let mut grid = Grid::default_office();

        // block row y=3 from x=1 to x=4 (except the desk at x=3 which is already there)
        // use a fresh approach: block x=2,3 at y=2 to force going around
        // actually let's block a column: x=3, y=1..=5 all wall except perimeter already clear
        // simpler: add walls at (2,2),(2,3),(2,4) to force going around column
        grid.tiles[2][2] = Tile::Wall;
        grid.tiles[3][2] = Tile::Wall;
        grid.tiles[4][2] = Tile::Wall;

        let from = Pos { x: 1, y: 3 };
        let to = Pos { x: 4, y: 3 };

        let path = pathfind(&grid, from, to).expect("should find detour path");
        assert!(!path.is_empty());
        assert_eq!(*path.last().unwrap(), to);

        // path must avoid the wall tiles
        for step in &path {
            assert!(
                !matches!(grid.tiles[step.y][step.x], Tile::Wall),
                "path stepped on wall at {:?}",
                step
            );
        }

        // detour means longer than direct (direct would be 3 steps)
        assert!(path.len() > 3);
    }

    #[test]
    fn test_pathfind_no_path() {
        let mut grid = Grid::default_office();

        // wall off a section: surround (5,5) with walls
        grid.tiles[4][5] = Tile::Wall;
        grid.tiles[6][5] = Tile::Wall;
        grid.tiles[5][4] = Tile::Wall;
        grid.tiles[5][6] = Tile::Wall;

        let from = Pos { x: 1, y: 1 };
        let to = Pos { x: 5, y: 5 };

        assert!(pathfind(&grid, from, to).is_none());
    }

    #[test]
    fn test_overflow_ring_exists() {
        let grid = Grid::default_office();
        assert!(
            grid.overflow_ring.len() >= 4,
            "expected at least 4 overflow ring tiles, got {}",
            grid.overflow_ring.len()
        );
    }
}
