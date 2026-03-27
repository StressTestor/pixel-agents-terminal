// Agent state machine

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Spawned,
    Walking,
    Typing,
    Reading,
    Idle,
    Waiting,
    Despawn,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    Init,
    ToolStart(String),
    ToolEnd,
    PermissionPrompt,
    Tick(Duration),
    DespawnTimeout,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SideEffect {
    RequestDeskPath,
    RequestWanderPath,
    RequestReturnPath,
    ShowBubble,
    HideBubble,
    SpawnSubAgent(String),
    ResetIdleTimer,
    FadeOut,
    FreeSeat,
}

const IDLE_TIMEOUT: Duration = Duration::from_secs(3);
const TYPING_TOOLS: &[&str] = &["Write", "Edit", "Bash", "NotebookEdit"];
const READING_TOOLS: &[&str] = &["Read", "Grep", "Glob", "WebFetch", "WebSearch"];
const SPAWN_TOOLS: &[&str] = &["Agent", "Task"];
const WAITING_TOOLS: &[&str] = &["AskUserQuestion"];

fn tool_to_state(tool: &str) -> AgentState {
    if TYPING_TOOLS.contains(&tool) {
        AgentState::Typing
    } else if READING_TOOLS.contains(&tool) {
        AgentState::Reading
    } else if SPAWN_TOOLS.contains(&tool) {
        AgentState::Walking
    } else if WAITING_TOOLS.contains(&tool) {
        AgentState::Waiting
    } else {
        AgentState::Idle
    }
}

pub fn transition(current: AgentState, event: AgentEvent) -> (AgentState, Vec<SideEffect>) {
    match event {
        // DespawnTimeout wins from any state
        AgentEvent::DespawnTimeout => {
            (AgentState::Despawn, vec![SideEffect::FadeOut, SideEffect::FreeSeat])
        }

        // PermissionPrompt wins from any state
        AgentEvent::PermissionPrompt => {
            (AgentState::Waiting, vec![SideEffect::ShowBubble])
        }

        AgentEvent::Init => {
            if current == AgentState::Spawned {
                (AgentState::Walking, vec![SideEffect::RequestDeskPath])
            } else {
                (current, vec![])
            }
        }

        AgentEvent::ToolStart(ref tool) => {
            let next_state = tool_to_state(tool);
            let mut effects: Vec<SideEffect> = vec![];

            let is_spawn_tool = SPAWN_TOOLS.contains(&tool.as_str());

            match current {
                AgentState::Waiting => {
                    effects.push(SideEffect::HideBubble);
                    if is_spawn_tool {
                        effects.push(SideEffect::SpawnSubAgent(tool.clone()));
                    }
                }
                AgentState::Walking => {
                    effects.push(SideEffect::RequestReturnPath);
                    if is_spawn_tool {
                        effects.push(SideEffect::SpawnSubAgent(tool.clone()));
                    }
                    // Walking stays Walking regardless of tool when mid-walk
                    return (AgentState::Walking, effects);
                }
                AgentState::Typing | AgentState::Reading | AgentState::Idle => {
                    if is_spawn_tool {
                        effects.push(SideEffect::SpawnSubAgent(tool.clone()));
                    }
                }
                _ => {}
            }

            (next_state, effects)
        }

        AgentEvent::ToolEnd => {
            match current {
                AgentState::Typing | AgentState::Reading => {
                    (AgentState::Idle, vec![SideEffect::ResetIdleTimer])
                }
                AgentState::Waiting => {
                    (AgentState::Idle, vec![SideEffect::HideBubble, SideEffect::ResetIdleTimer])
                }
                // ToolEnd when already Idle or other states: no-op
                _ => (current, vec![]),
            }
        }

        AgentEvent::Tick(duration) => {
            if current == AgentState::Idle && duration >= IDLE_TIMEOUT {
                (AgentState::Walking, vec![SideEffect::RequestWanderPath])
            } else {
                (current, vec![])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // 1. Spawned + Init -> Walking + [RequestDeskPath]
    #[test]
    fn test_spawn_to_walking() {
        let (state, effects) = transition(AgentState::Spawned, AgentEvent::Init);
        assert_eq!(state, AgentState::Walking);
        assert_eq!(effects, vec![SideEffect::RequestDeskPath]);
    }

    // 2. Idle + ToolStart("Write") -> Typing
    #[test]
    fn test_idle_to_typing() {
        let (state, effects) =
            transition(AgentState::Idle, AgentEvent::ToolStart("Write".into()));
        assert_eq!(state, AgentState::Typing);
        assert!(effects.is_empty());
    }

    // 3. Idle + ToolStart("Read") -> Reading
    #[test]
    fn test_idle_to_reading() {
        let (state, effects) =
            transition(AgentState::Idle, AgentEvent::ToolStart("Read".into()));
        assert_eq!(state, AgentState::Reading);
        assert!(effects.is_empty());
    }

    // 4. Typing + ToolEnd -> Idle + [ResetIdleTimer]
    #[test]
    fn test_typing_to_idle_on_tool_end() {
        let (state, effects) = transition(AgentState::Typing, AgentEvent::ToolEnd);
        assert_eq!(state, AgentState::Idle);
        assert_eq!(effects, vec![SideEffect::ResetIdleTimer]);
    }

    // 5. Idle + Tick(3s) -> Walking + [RequestWanderPath]
    #[test]
    fn test_idle_timeout_to_walking() {
        let (state, effects) =
            transition(AgentState::Idle, AgentEvent::Tick(Duration::from_secs(3)));
        assert_eq!(state, AgentState::Walking);
        assert_eq!(effects, vec![SideEffect::RequestWanderPath]);
    }

    // 6. Idle + Tick(1s) -> Idle + [] (no change)
    #[test]
    fn test_idle_no_timeout_yet() {
        let (state, effects) =
            transition(AgentState::Idle, AgentEvent::Tick(Duration::from_secs(1)));
        assert_eq!(state, AgentState::Idle);
        assert!(effects.is_empty());
    }

    // 7. Walking + ToolStart("Write") -> Walking + [RequestReturnPath]
    #[test]
    fn test_walking_to_typing_on_tool_event() {
        let (state, effects) =
            transition(AgentState::Walking, AgentEvent::ToolStart("Write".into()));
        assert_eq!(state, AgentState::Walking);
        assert_eq!(effects, vec![SideEffect::RequestReturnPath]);
    }

    // 8. Any + PermissionPrompt -> Waiting + [ShowBubble]
    #[test]
    fn test_any_to_waiting() {
        for start in [
            AgentState::Spawned,
            AgentState::Walking,
            AgentState::Typing,
            AgentState::Reading,
            AgentState::Idle,
        ] {
            let (state, effects) = transition(start, AgentEvent::PermissionPrompt);
            assert_eq!(state, AgentState::Waiting, "failed from {start:?}");
            assert_eq!(effects, vec![SideEffect::ShowBubble], "failed from {start:?}");
        }
    }

    // 9. Waiting + ToolStart("Edit") -> Typing + [HideBubble]
    #[test]
    fn test_waiting_to_typing() {
        let (state, effects) =
            transition(AgentState::Waiting, AgentEvent::ToolStart("Edit".into()));
        assert_eq!(state, AgentState::Typing);
        assert_eq!(effects, vec![SideEffect::HideBubble]);
    }

    // 10. Any + DespawnTimeout -> Despawn + [FadeOut, FreeSeat]
    #[test]
    fn test_despawn() {
        for start in [
            AgentState::Spawned,
            AgentState::Walking,
            AgentState::Typing,
            AgentState::Reading,
            AgentState::Idle,
            AgentState::Waiting,
        ] {
            let (state, effects) = transition(start, AgentEvent::DespawnTimeout);
            assert_eq!(state, AgentState::Despawn, "failed from {start:?}");
            assert_eq!(
                effects,
                vec![SideEffect::FadeOut, SideEffect::FreeSeat],
                "failed from {start:?}"
            );
        }
    }

    // 11. Typing + ToolStart("Read") -> Reading (rapid tool switch)
    #[test]
    fn test_rapid_tool_switch() {
        let (state, effects) =
            transition(AgentState::Typing, AgentEvent::ToolStart("Read".into()));
        assert_eq!(state, AgentState::Reading);
        assert!(effects.is_empty());
    }

    // 12. Idle + ToolEnd -> Idle (no-op)
    #[test]
    fn test_tool_end_when_idle_is_noop() {
        let (state, effects) = transition(AgentState::Idle, AgentEvent::ToolEnd);
        assert_eq!(state, AgentState::Idle);
        assert!(effects.is_empty());
    }

    // 13. Idle + ToolStart("Agent") -> Walking + [SpawnSubAgent("Agent")]
    #[test]
    fn test_agent_tool_spawns_child() {
        let (state, effects) =
            transition(AgentState::Idle, AgentEvent::ToolStart("Agent".into()));
        assert_eq!(state, AgentState::Walking);
        assert_eq!(effects, vec![SideEffect::SpawnSubAgent("Agent".into())]);
    }

    // 14. Idle + ToolStart("SomeFutureTool") -> Idle (unknown tool maps to Idle)
    #[test]
    fn test_unknown_tool_maps_to_idle() {
        let (state, effects) =
            transition(AgentState::Idle, AgentEvent::ToolStart("SomeFutureTool".into()));
        assert_eq!(state, AgentState::Idle);
        assert!(effects.is_empty());
    }
}
