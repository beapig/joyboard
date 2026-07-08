/// 状态文件读写，用于 daemon → TUI/overlay 进程通信
///
/// daemon 每帧写入 `/tmp/joyboard-state.json`，
/// TUI 和 overlay 进程独立读取渲染。

use crate::engine::{EngineState, WorkMode};
use crate::engine::layer::Layer;
use serde::{Deserialize, Serialize};

const STATE_FILE: &str = "/tmp/joyboard-state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePayload {
    pub mode: String,
    pub layer: String,
    pub left_grid_selected: Option<usize>,
    pub right_grid_selected: Option<usize>,
    pub left_joystick: [f32; 2],
    pub right_joystick: [f32; 2],
    pub shift: bool,
    pub capslock: bool,
}

/// 主进程写入状态文件（无 feature 依赖，始终可调用）
pub fn write(state: &EngineState) {
    let payload = StatePayload {
        mode: match state.mode {
            WorkMode::Keyboard => "Keyboard".into(),
            WorkMode::Mouse => "Mouse".into(),
            WorkMode::Gamepad => "Gamepad".into(),
        },
        layer: match state.layer {
            Layer::Base => "Base".into(),
            Layer::Fn => "Fn".into(),
        },
        left_grid_selected: state.left_grid_selected,
        right_grid_selected: state.right_grid_selected,
        left_joystick: [state.left_joystick.0, state.left_joystick.1],
        right_joystick: [state.right_joystick.0, state.right_joystick.1],
        shift: state.shift_pressed,
        capslock: state.capslock_activated,
    };

    if let Ok(json) = serde_json::to_string(&payload) {
        let tmp = format!("{}.tmp", STATE_FILE);
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, STATE_FILE);
        }
    }
}

/// TUI/overlay 进程读取状态文件
pub fn read() -> Option<StatePayload> {
    std::fs::read_to_string(STATE_FILE).ok().and_then(|s| {
        serde_json::from_str(&s).ok()
    })
}
