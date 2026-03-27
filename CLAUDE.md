## pixel-agents-terminal

animated pixel-art characters in Ghostty reflecting Claude Code agent state.

### commands

```bash
cargo build          # build
cargo test           # run all tests
cargo run            # run (must be in Ghostty)
cargo run -- --help  # show CLI options
```

### architecture

async event pipeline: tokio + notify + mpsc channels.
transcript watcher -> state manager -> scene compositor -> kitty renderer.
raw RGBA pixels via Kitty graphics protocol (no PNG encoding).
DECSC/delete-all/transmit/DECRC pattern per Kovid Goyal recommendation.

### key files

- `src/agent.rs` - pure FSM: transition(state, event, elapsed) -> (state, side_effects)
- `src/transcript.rs` - JSONL parser for Claude Code transcript files
- `src/grid.rs` - 16x12 tile grid, BFS pathfinding, desk assignment
- `src/renderer.rs` - Kitty protocol escape sequence generation + chunking
- `src/scene.rs` - composite agents + furniture into ImageBuffer
- `src/sprites.rs` - procedural colored rectangle sprites
- `src/watcher.rs` - notify-based file watcher with byte offset tracking
- `src/ui.rs` - crossterm status bar + keyboard input
- `src/main.rs` - tokio runtime, select! loop, CLI args

### kitty protocol notes

- image ID replacement is broken in Ghostty (issue #6711)
- use DECSC + delete-all + a=T transmit + DECRC pattern
- raw RGBA (f=32), not PNG (f=100)
- chunk base64 at 4096 bytes per escape sequence
- q=2 suppresses terminal responses, C=1 suppresses cursor movement
