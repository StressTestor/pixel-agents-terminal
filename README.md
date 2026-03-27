# pixel-agents-terminal

little pixel people walking around an office in your terminal, reflecting what your Claude Code agents are actually doing.

## the problem

Claude Code gives you no visual feedback on agent state. you either stare at scrolling text or open a separate app. Agora needs its own window. pixel-agents (the VS Code extension) is VS Code-only. i just wanted something i could drop in a terminal split and forget about.

## what it looks like

you get a 16x12 grid that renders directly in your terminal via the Kitty graphics protocol. each agent is a colored pixel character. idle agents sit at their desks. working agents walk to a task. agents using tools light up differently. no external window, no browser, just raw RGBA pixels painted into the terminal at whatever FPS you set.

## requirements

- Ghostty (Kitty protocol support required, no Sixel fallback yet)
- Rust toolchain (stable, 2021 edition)

## install

```bash
cargo build --release
# binary at: target/release/pixel-agents-terminal

# or install globally
cargo install --path .
```

## usage

```bash
pixel-agents-terminal
pixel-agents-terminal --project ~/.claude
pixel-agents-terminal --fps 15
pixel-agents-terminal --fps 5 --scale 1
```

defaults to watching `~/.claude` for transcript files. `--scale` is reserved, does nothing yet.

## controls

| key | action |
|-----|--------|
| `q` / `Esc` | quit |
| `Space` | pause |
| `r` | refresh |

## how it works

watches Claude Code JSONL transcript files with byte-offset tracking so it only reads new lines. each agent runs a pure FSM: `transition(state, event, elapsed) -> (state, side_effects)`. agents navigate the 16x12 tile grid via BFS. every frame, the scene compositor builds a flat `ImageBuffer`, which gets written to the terminal as raw RGBA (not PNG) via the Kitty graphics protocol.

the render pattern is DECSC, delete-all, transmit, DECRC, per Kovid Goyal's recommendation. avoids Ghostty's broken image-ID replacement (issue #6711). base64 chunks at 4096 bytes per escape sequence. `q=2` suppresses responses, `C=1` suppresses cursor movement.

## known limitations

- sprites are procedural colored rectangles, not real pixel art yet
- single project directory at a time
- Ghostty only, no Sixel fallback for other terminals
