// Main event loop, terminal management, agent lifecycle

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute, terminal,
};

use crate::agent::{self, AgentEvent as FsmEvent, AgentState, SideEffect};
use crate::grid::{pathfind, Grid, Pos, GRID_HEIGHT, TILE_SIZE};
use crate::renderer;
use crate::scene::{self, AgentView};
use crate::transcript::TranscriptEvent;
use crate::watcher::TranscriptReader;

/// Hue palette for up to 8 agents (evenly spaced around the color wheel)
const AGENT_HUES: [f32; 8] = [200.0, 30.0, 120.0, 280.0, 50.0, 330.0, 170.0, 80.0];

/// Ticks between path advances (controls walk speed)
const WALK_SPEED_TICKS: u32 = 3;

/// Seconds of inactivity before idle wander triggers
const IDLE_WANDER_SECS: u64 = 3;

/// Seconds of total inactivity before despawn
const DESPAWN_TIMEOUT_SECS: u64 = 60;

/// Spawn point: just inside the door (bottom-center of office)
const SPAWN_POS: Pos = Pos { x: 8, y: 10 };

struct AgentInstance {
    state: AgentState,
    pos: Pos,
    hue: f32,
    desk: Option<usize>,
    path: Vec<Pos>,
    path_index: usize,
    idle_since: Instant,
    last_activity: Instant,
    direction: u8,
    frame: u32,
    frame_counter: u32,
    reader: TranscriptReader,
}

/// Walk `root/projects/*/sessions/*/transcript.jsonl` and return all matches.
fn glob_transcripts(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    let projects_dir = root.join("projects");
    if !projects_dir.is_dir() {
        return Ok(results);
    }

    for project_entry in std::fs::read_dir(&projects_dir)? {
        let project_entry = project_entry?;
        if !project_entry.file_type()?.is_dir() {
            continue;
        }

        // Claude Code stores sessions as {uuid}.jsonl directly under the project dir
        // (not in a sessions/ subdirectory)
        for file_entry in std::fs::read_dir(project_entry.path())? {
            let file_entry = file_entry?;
            let path = file_entry.path();

            // Match *.jsonl files (session transcripts are {uuid}.jsonl)
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if !path.is_file() {
                continue;
            }

            // Only include files with content (prevents phantom agents)
            if let Ok(meta) = std::fs::metadata(&path) {
                if meta.len() > 0 {
                    // Only include recently active files (modified in last 10 minutes)
                    // to avoid spawning agents for every old session
                    if let Ok(modified) = meta.modified() {
                        if let Ok(elapsed) = modified.elapsed() {
                            if elapsed.as_secs() < 600 {
                                results.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Map a TranscriptEvent to the FSM's AgentEvent
fn transcript_to_fsm(evt: &TranscriptEvent) -> FsmEvent {
    match evt {
        TranscriptEvent::ToolStart { tool_name, .. } => {
            FsmEvent::ToolStart(tool_name.clone())
        }
        TranscriptEvent::ToolEnd { .. } => FsmEvent::ToolEnd,
        TranscriptEvent::PermissionPrompt { .. } => FsmEvent::PermissionPrompt,
        TranscriptEvent::TurnEnd => FsmEvent::ToolEnd,
    }
}

/// Compute direction (0=down, 1=up, 2=right, 3=left) from movement delta
fn direction_from_delta(from: Pos, to: Pos) -> u8 {
    let dx = to.x as i32 - from.x as i32;
    let dy = to.y as i32 - from.y as i32;

    if dy.abs() >= dx.abs() {
        if dy > 0 { 0 } else { 1 }
    } else if dx > 0 {
        2
    } else {
        3
    }
}

/// Pick a random-ish overflow tile based on current tick
fn pick_overflow_tile(grid: &Grid, tick: u64) -> Pos {
    if grid.overflow_ring.is_empty() {
        // Fallback: center of the office
        return Pos { x: 8, y: 6 };
    }
    let idx = (tick as usize) % grid.overflow_ring.len();
    grid.overflow_ring[idx]
}

pub async fn run(
    project: Option<PathBuf>,
    fps: u32,
    _scale: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let watch_root = match project {
        Some(p) => p,
        None => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".claude")
        }
    };

    // Enter alternate screen, raw mode, hide cursor
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide
    )?;
    terminal::enable_raw_mode()?;

    let grid = Grid::default_office();
    let mut agents: HashMap<String, AgentInstance> = HashMap::new();
    let mut desk_taken: [bool; 6] = [false; 6];
    let mut generation: u64 = 0;
    let mut last_rendered: u64 = 0;
    let mut paused = false;
    let mut tick_count: u64 = 0;
    let mut agent_counter: usize = 0; // for hue assignment

    let tick_duration = Duration::from_millis(1000 / fps.max(1) as u64);
    let mut interval = tokio::time::interval(tick_duration);

    // Ctrl+C handler
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let result: Result<(), Box<dyn std::error::Error>> = loop {
        tokio::select! {
            _ = interval.tick() => {},
            _ = &mut ctrl_c => {
                break Ok(());
            }
        }

        // Poll keyboard events (non-blocking)
        let mut should_break = false;
        let mut force_render = false;

        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(key_event) => {
                    // Ctrl+C as backup
                    if key_event.modifiers.contains(KeyModifiers::CONTROL)
                        && key_event.code == KeyCode::Char('c')
                    {
                        should_break = true;
                        break;
                    }
                    match key_event.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            should_break = true;
                            break;
                        }
                        KeyCode::Char('r') => {
                            force_render = true;
                        }
                        KeyCode::Char(' ') => {
                            paused = !paused;
                            force_render = true; // re-render to show pause state
                        }
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {
                    force_render = true;
                }
                _ => {}
            }
        }

        if should_break {
            break Ok(());
        }

        if paused {
            if force_render {
                generation += 1;
            }
            // Still render if forced, but skip simulation
            if generation > last_rendered || force_render {
                render_scene(&grid, &agents, &mut stdout, paused, fps)?;
                last_rendered = generation;
            }
            continue;
        }

        tick_count += 1;
        let mut changed = false;

        // --- Scan for new transcript files ---
        if tick_count % (fps.max(1) as u64 * 2) == 0 {
            // Scan every 2 seconds worth of ticks
            if let Ok(transcripts) = glob_transcripts(&watch_root) {
                for path in transcripts {
                    let key = path.to_string_lossy().to_string();
                    if agents.contains_key(&key) {
                        continue;
                    }

                    // Find nearest free desk
                    let desk = desk_taken.iter().position(|&taken| !taken);
                    if let Some(desk_idx) = desk {
                        desk_taken[desk_idx] = true;
                    }

                    let hue = AGENT_HUES[agent_counter % AGENT_HUES.len()];
                    agent_counter += 1;
                    let now = Instant::now();

                    let target_pos = match desk {
                        Some(idx) => grid.desk_positions[idx],
                        None => pick_overflow_tile(&grid, tick_count),
                    };

                    let path_to_desk = pathfind(&grid, SPAWN_POS, target_pos)
                        .unwrap_or_default();

                    agents.insert(
                        key,
                        AgentInstance {
                            state: AgentState::Walking,
                            pos: SPAWN_POS,
                            hue,
                            desk,
                            path: path_to_desk,
                            path_index: 0,
                            idle_since: now,
                            last_activity: now,
                            direction: 1, // facing up (walking into office)
                            frame: 0,
                            frame_counter: 0,
                            reader: TranscriptReader::new(path),
                        },
                    );
                    changed = true;
                }
            }
        }

        // --- Process transcript events for each agent ---
        let agent_keys: Vec<String> = agents.keys().cloned().collect();
        for key in &agent_keys {
            let agent = match agents.get_mut(key) {
                Some(a) => a,
                None => continue,
            };

            // Skip despawned agents
            if agent.state == AgentState::Despawn {
                continue;
            }

            let events = agent.reader.read_new_events();
            for evt in &events {
                let fsm_event = transcript_to_fsm(evt);

                let (new_state, effects) = agent::transition(agent.state, fsm_event);
                agent.state = new_state;
                agent.last_activity = Instant::now();

                for effect in effects {
                    handle_side_effect(agent, &effect, &grid, tick_count);
                }
                changed = true;
            }

            // --- Check despawn timeout ---
            if agent.last_activity.elapsed() >= Duration::from_secs(DESPAWN_TIMEOUT_SECS) {
                let (new_state, effects) =
                    agent::transition(agent.state, FsmEvent::DespawnTimeout);
                agent.state = new_state;
                for effect in effects {
                    handle_side_effect(agent, &effect, &grid, tick_count);
                }
                changed = true;
            }

            // --- Check idle wander timeout ---
            if agent.state == AgentState::Idle
                && agent.idle_since.elapsed() >= Duration::from_secs(IDLE_WANDER_SECS)
            {
                let (new_state, effects) = agent::transition(
                    agent.state,
                    FsmEvent::Tick(Duration::from_secs(IDLE_WANDER_SECS)),
                );
                agent.state = new_state;
                for effect in effects {
                    handle_side_effect(agent, &effect, &grid, tick_count);
                }
                changed = true;
            }

            // --- Advance walking ---
            if agent.state == AgentState::Walking && !agent.path.is_empty() {
                agent.frame_counter += 1;
                if agent.frame_counter >= WALK_SPEED_TICKS {
                    agent.frame_counter = 0;

                    if agent.path_index < agent.path.len() {
                        let next = agent.path[agent.path_index];
                        agent.direction = direction_from_delta(agent.pos, next);
                        agent.pos = next;
                        agent.path_index += 1;
                        agent.frame = (agent.frame + 1) % 3;
                        changed = true;
                    }

                    // Path exhausted -> go idle
                    if agent.path_index >= agent.path.len() {
                        agent.path.clear();
                        agent.path_index = 0;
                        agent.state = AgentState::Idle;
                        agent.idle_since = Instant::now();
                        agent.frame = 0;
                        changed = true;
                    }
                }
            }
        }

        // --- Remove despawned agents ---
        let to_remove: Vec<String> = agents
            .iter()
            .filter(|(_, a)| a.state == AgentState::Despawn)
            .map(|(k, _)| k.clone())
            .collect();

        for key in to_remove {
            if let Some(agent) = agents.remove(&key) {
                if let Some(desk_idx) = agent.desk {
                    if desk_idx < desk_taken.len() {
                        desk_taken[desk_idx] = false;
                    }
                }
                changed = true;
            }
        }

        if changed || force_render {
            generation += 1;
        }

        if generation > last_rendered {
            render_scene(&grid, &agents, &mut stdout, paused, fps)?;
            last_rendered = generation;
        }
    };

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(
        stdout,
        terminal::LeaveAlternateScreen,
        cursor::Show
    )?;

    result
}

fn handle_side_effect(
    agent: &mut AgentInstance,
    effect: &SideEffect,
    grid: &Grid,
    tick_count: u64,
) {
    match effect {
        SideEffect::RequestDeskPath => {
            if let Some(desk_idx) = agent.desk {
                let target = grid.desk_positions[desk_idx];
                if let Some(path) = pathfind(grid, agent.pos, target) {
                    agent.path = path;
                    agent.path_index = 0;
                    agent.frame_counter = 0;
                }
            }
        }
        SideEffect::RequestReturnPath => {
            if let Some(desk_idx) = agent.desk {
                let target = grid.desk_positions[desk_idx];
                if let Some(path) = pathfind(grid, agent.pos, target) {
                    agent.path = path;
                    agent.path_index = 0;
                    agent.frame_counter = 0;
                }
            }
        }
        SideEffect::RequestWanderPath => {
            let target = pick_overflow_tile(grid, tick_count);
            if let Some(path) = pathfind(grid, agent.pos, target) {
                agent.path = path;
                agent.path_index = 0;
                agent.frame_counter = 0;
            }
        }
        SideEffect::ResetIdleTimer => {
            agent.idle_since = Instant::now();
        }
        SideEffect::FreeSeat => {
            // Desk freeing handled in the removal loop
        }
        SideEffect::FadeOut => {
            // Visual fade not implemented yet - agent just gets removed
        }
        SideEffect::ShowBubble | SideEffect::HideBubble => {
            // Bubble rendering not implemented yet - state overlay in sprite handles it
        }
        SideEffect::SpawnSubAgent(_) => {
            // Sub-agent spawning handled at the main loop level, not here
        }
    }
}

fn render_scene(
    grid: &Grid,
    agents: &HashMap<String, AgentInstance>,
    stdout: &mut io::Stdout,
    paused: bool,
    fps: u32,
) -> io::Result<()> {
    // Build agent views
    let agent_views: Vec<AgentView> = agents
        .values()
        .filter(|a| a.state != AgentState::Despawn)
        .map(|a| AgentView {
            pos: a.pos,
            hue: a.hue,
            state: a.state,
            frame: a.frame,
            direction: a.direction,
        })
        .collect();

    // Composite and render
    let img = scene::composite_scene(grid, &agent_views);
    let frame_bytes = renderer::render_frame(&img);
    stdout.write_all(&frame_bytes)?;

    // Status bar below the image
    // Image height in terminal rows: pixel_height / cell_height
    // Kitty places the image at cursor; we move below it
    let image_rows = (GRID_HEIGHT as u32 * TILE_SIZE) / 16 + 1; // rough estimate: 16px per cell row
    execute!(
        stdout,
        cursor::MoveTo(0, image_rows as u16)
    )?;

    // Count states
    let total = agents.len();
    let typing = agents.values().filter(|a| a.state == AgentState::Typing).count();
    let reading = agents.values().filter(|a| a.state == AgentState::Reading).count();
    let waiting = agents.values().filter(|a| a.state == AgentState::Waiting).count();

    let pause_indicator = if paused { " [PAUSED]" } else { "" };

    // Clear line and write status
    write!(
        stdout,
        "\x1b[2K agents: {} | typing: {} | reading: {} | waiting: {} | fps: {}{}",
        total, typing, reading, waiting, fps, pause_indicator
    )?;

    stdout.flush()?;
    Ok(())
}
