use crate::config::keymap::{parse_key_name, BASE_LAYOUT, FN_LAYOUT};
use crate::config::Config;
use crate::engine::layer::{Layer, LayerManager};
use crate::engine::se_st::{SeStAction, SeStPair, SeStState};
use crate::input::GamepadEvent;
use crate::mouse::MouseEngine;
use crate::output::Action;
use std::collections::HashMap;
use std::time::Instant;

pub mod grid;
pub mod layer;
pub mod se_st;

// ANBERNIC-keys 设备按键码映射（物理按键 → evdev code）
pub const BTN_A: u32 = 304;      // A (SOUTH)
pub const BTN_B: u32 = 305;      // B (EAST)
pub const BTN_X: u32 = 307;      // X (NORTH)
pub const BTN_Y: u32 = 306;      // Y (C)
pub const BTN_L1: u32 = 308;     // L1 (WEST)
pub const BTN_L2: u32 = 314;     // L2 (SELECT)
pub const BTN_L3: u32 = 313;     // L3 (TR2)
pub const BTN_R1: u32 = 309;     // R1 (Z)
pub const BTN_R2: u32 = 315;     // R2 (START)
pub const BTN_R3: u32 = 316;     // R3 (MODE)
pub const BTN_SE: u32 = 310;     // SE (TL)
pub const BTN_ST: u32 = 311;     // ST (TR)
pub const BTN_FN: u32 = 312;     // Menu (TL2)
pub const BTN_VOL_UP: u32 = 115;
pub const BTN_VOL_DOWN: u32 = 114;

/// 工作模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorkMode {
    Keyboard,
    Mouse,
    Gamepad,
}

/// 引擎状态（用于 TUI 展示）
#[derive(Debug)]
pub struct EngineState {
    pub mode: WorkMode,
    pub layer: Layer,
    pub left_grid_selected: Option<usize>,
    pub right_grid_selected: Option<usize>,
    pub left_joystick: (f32, f32),
    pub right_joystick: (f32, f32),
}

/// 事件引擎
pub struct EventEngine {
    pub mode: WorkMode,
    prev_mode: WorkMode,
    layer: LayerManager,
    se_st: SeStPair,
    left_grid: grid::JoystickGrid,
    right_grid: grid::JoystickGrid,
    left_joy: (f32, f32),
    right_joy: (f32, f32),
    left_cell_locked: Option<usize>,
    right_cell_locked: Option<usize>,
    tap_threshold_ms: u64,
    hold_threshold_ms: u64,
    double_tap_interval_ms: u64,
    button_roles: HashMap<u32, &'static str>,
    fn_pressed_at: Option<Instant>,
    mouse: MouseEngine,
    mouse_last_l: (f32, f32),
    mouse_last_r: (f32, f32),
    /// 滚轮累加器，(x, y) 分别对应右摇杆水平和垂直
    scroll_accum: (f32, f32),
    /// 滚轮起手标记，(x, y) 分别对应两个轴向
    scroll_primed: (bool, bool),
    /// 鼠标精细模式子像素累加器
    mouse_accum: (f64, f64),
    /// 鼠标精细模式起手标记
    mouse_fine_primed: bool,
    /// L3 释放后精细模式的退出时刻（延迟 500ms）
    fine_exit_at: Option<Instant>,
    /// L1 按下标记：L1 激活精细模式时不响应 L3 的延迟退出定时器
    fine_l1_active: bool,
}

impl EventEngine {
    pub fn new(config: &Config) -> Self {
        let tap = config.button_mode.tap_threshold_ms;
        let mut button_roles = HashMap::new();
        button_roles.insert(BTN_A, "A→Enter");
        button_roles.insert(BTN_B, "B→Esc");
        button_roles.insert(BTN_X, "X→Delete");
        button_roles.insert(BTN_Y, "Y→Insert");
        button_roles.insert(BTN_L1, "L1→FN");
        button_roles.insert(BTN_L2, "L2→Ctrl");
        button_roles.insert(BTN_R1, "R1→Backspace");
        button_roles.insert(BTN_R2, "R2→Alt");
        button_roles.insert(BTN_L3, "L3→[grid]");
        button_roles.insert(BTN_R3, "R3→[grid]");
        button_roles.insert(BTN_SE, "SE→Tab/Shift");
        button_roles.insert(BTN_ST, "ST→Space/Shift");
        button_roles.insert(BTN_FN, "FN→Mode");
        button_roles.insert(103, "D-pad↑");
        button_roles.insert(108, "D-pad↓");
        button_roles.insert(105, "D-pad←");
        button_roles.insert(106, "D-pad→");
        Self {
            mode: WorkMode::Keyboard,
            prev_mode: WorkMode::Keyboard,
            layer: LayerManager::new(),
            se_st: SeStPair::new(tap, config.button_mode.hold_threshold_ms),
            left_grid: grid::JoystickGrid::new(&config.joystick_grid.left),
            right_grid: grid::JoystickGrid::new(&config.joystick_grid.right),
            left_joy: (0.0, 0.0),
            right_joy: (0.0, 0.0),
            left_cell_locked: None,
            right_cell_locked: None,
            tap_threshold_ms: tap,
            hold_threshold_ms: config.button_mode.hold_threshold_ms,
            double_tap_interval_ms: config.button_mode.double_tap_interval_ms,
            button_roles,
            fn_pressed_at: None,
            mouse: MouseEngine::new(config.mouse.sensitivity, config.mouse.fine_control.dpi_scale),
            mouse_last_l: (0.0, 0.0),
            mouse_last_r: (0.0, 0.0),
            scroll_accum: (0.0, 0.0),
            scroll_primed: (false, false),
            mouse_accum: (0.0, 0.0),
            mouse_fine_primed: false,
            fine_exit_at: None,
            fine_l1_active: false,
        }
    }

    pub fn state(&self) -> EngineState {
        EngineState {
            mode: self.mode,
            layer: self.layer.current(),
            left_grid_selected: self.left_cell_locked.or_else(|| self.left_grid.selected_cell(self.left_joy)),
            right_grid_selected: self.right_cell_locked.or_else(|| self.right_grid.selected_cell(self.right_joy)),
            left_joystick: self.left_joy,
            right_joystick: self.right_joy,
        }
    }

    /// 输入事件 → 动作列表
    pub fn feed(&mut self, events: Vec<GamepadEvent>) -> Vec<Action> {
        let mut actions = Vec::new();

        for event in events {
            match event {
                GamepadEvent::ButtonDown { code } => {
                    self.on_button_down(code, &mut actions);
                }
                GamepadEvent::ButtonUp { code } => {
                    self.on_button_up(code, &mut actions);
                }
                GamepadEvent::AxisMotion { axis, value } => {
                    self.on_axis(axis, value);
                }
            }
        }

        // 每帧检查 SE/ST 长按 → ShiftDown（必须 hold_threshold 后持续按住）
        let se_st_actions = self.se_st.tick();
        self.actions_from_se_st(se_st_actions, &mut actions);

        // 鼠标模式：摇杆持续移动（不依赖轴事件的触发频率）
        if self.mode == WorkMode::Mouse {
            // 检查 L3 释放后的延迟退出定时器（L1 保持时跳过）
            if !self.fine_l1_active {
                if let Some(exit_at) = self.fine_exit_at {
                    if Instant::now() >= exit_at {
                        self.mouse.set_fine_mode(false);
                        self.fine_exit_at = None;
                        self.mouse_accum = (0.0, 0.0);
                        self.mouse_fine_primed = false;
                    }
                }
            }
            self.emit_mouse_motion(&mut actions);
        }

        actions
    }

    fn on_button_down(&mut self, code: u32, actions: &mut Vec<Action>) {
        match code {
            BTN_VOL_UP | BTN_VOL_DOWN => {
                // 音量键透传到系统
                actions.push(Action::KeyDown(code as u16));
            }
            BTN_FN => {
                self.fn_pressed_at = Some(Instant::now());
                if self.mode == WorkMode::Gamepad {
                    // 手柄模式下：透传为普通按键
                    actions.push(Action::KeyDown(BTN_FN as u16));
                }
            }
            BTN_L1 => {
                self.on_l1_down();
            }
            BTN_L3 => {
                if self.mode == WorkMode::Keyboard {
                    let cell = self.left_grid.selected_cell(self.left_joy);
                    self.left_cell_locked = cell;
                    if let Some(cell_idx) = cell {
                        let layout = match self.layer.current() {
                            Layer::Base => &BASE_LAYOUT,
                            Layer::Fn => &FN_LAYOUT,
                        };
                        if let Some(key_name) = get_key_name(layout, cell_idx, false) {
                            if let Some((key_code, _mods)) = parse_key_name(key_name) {
                                actions.push(Action::KeyDown(key_code));
                            }
                        }
                    }
                } else if self.mode == WorkMode::Mouse {
                    self.fine_exit_at = None;
                    self.mouse.set_fine_mode(true);
                }
            }
            BTN_R3 => {
                if self.mode == WorkMode::Keyboard {
                    let cell = self.right_grid.selected_cell(self.right_joy);
                    self.right_cell_locked = cell;
                    if let Some(cell_idx) = cell {
                        let layout = match self.layer.current() {
                            Layer::Base => &BASE_LAYOUT,
                            Layer::Fn => &FN_LAYOUT,
                        };
                        if let Some(key_name) = get_key_name(layout, cell_idx, true) {
                            if let Some((key_code, _mods)) = parse_key_name(key_name) {
                                actions.push(Action::KeyDown(key_code));
                            }
                        }
                    }
                } else if self.mode == WorkMode::Mouse {
                    actions.push(Action::KeyDown(274)); // BTN_MIDDLE
                }
            }
            BTN_SE => {
                let se_actions = self.se_st.on_se_down(Instant::now());
                self.actions_from_se_st(se_actions, actions);
            }
            BTN_ST => {
                let st_actions = self.se_st.on_st_down(Instant::now());
                self.actions_from_se_st(st_actions, actions);
            }
            // D-pad 方向键
            103 | 105 | 106 | 108 => {
                if self.mode != WorkMode::Gamepad && self.layer.current() == Layer::Fn {
                    // FN 层：映射为 PageUp/Down/Home/End
                    let fn_key = match code {
                        103 => 104, // Up → PageUp
                        108 => 109, // Down → PageDown
                        105 => 102, // Left → Home
                        106 => 107, // Right → End
                        _ => code,
                    };
                    actions.push(Action::KeyDown(fn_key as u16));
                } else {
                    self.emit_dpad(code, true, actions);
                }
            }
            _ => {
                // 手柄模式：所有按键透传
                if self.mode == WorkMode::Gamepad {
                    actions.push(Action::KeyDown(code as u16));
                    return;
                }
                // 第三方按键: 触发 SE/ST 的 Shift 提前
                let se_st_shift = self.se_st.on_third_party_down();
                self.actions_from_se_st(se_st_shift, actions);

                if self.mode == WorkMode::Keyboard {
                    self.on_keyboard_button_down(code, actions);
                } else if self.mode == WorkMode::Mouse {
                    self.on_mouse_button_down(code, actions);
                }
            }
        }
    }

    fn on_button_up(&mut self, code: u32, actions: &mut Vec<Action>) {
        match code {
            BTN_VOL_UP | BTN_VOL_DOWN => {
                actions.push(Action::KeyUp(code as u16));
            }
            BTN_L1 => self.on_l1_up(),
            BTN_L3 => {
                if self.mode == WorkMode::Keyboard {
                    if let Some(cell_idx) = self.left_cell_locked {
                        let layout = match self.layer.current() {
                            Layer::Base => &BASE_LAYOUT,
                            Layer::Fn => &FN_LAYOUT,
                        };
                        if let Some(key_name) = get_key_name(layout, cell_idx, false) {
                            if let Some((key_code, _mods)) = parse_key_name(key_name) {
                                actions.push(Action::KeyUp(key_code));
                            }
                        }
                    }
                    self.left_cell_locked = None;
                } else if self.mode == WorkMode::Mouse {
                    self.fine_exit_at = Some(Instant::now() + std::time::Duration::from_millis(500));
                }
            }
            BTN_R3 => {
                if self.mode == WorkMode::Keyboard {
                    if let Some(cell_idx) = self.right_cell_locked {
                        let layout = match self.layer.current() {
                            Layer::Base => &BASE_LAYOUT,
                            Layer::Fn => &FN_LAYOUT,
                        };
                        if let Some(key_name) = get_key_name(layout, cell_idx, true) {
                            if let Some((key_code, _mods)) = parse_key_name(key_name) {
                                actions.push(Action::KeyUp(key_code));
                            }
                        }
                    }
                    self.right_cell_locked = None;
                } else if self.mode == WorkMode::Mouse {
                    actions.push(Action::KeyUp(274));
                }
            }
            BTN_FN => {
                self.on_function_up(actions);
            }
            BTN_SE => {
                let se_actions = self.se_st.on_se_up();
                self.actions_from_se_st(se_actions, actions);
            }
            BTN_ST => {
                let st_actions = self.se_st.on_st_up();
                self.actions_from_se_st(st_actions, actions);
            }
            // D-pad 方向键
            103 | 105 | 106 | 108 => {
                if self.mode != WorkMode::Gamepad && self.layer.current() == Layer::Fn {
                    let fn_key = match code {
                        103 => 104,
                        108 => 109,
                        105 => 102,
                        106 => 107,
                        _ => code,
                    };
                    actions.push(Action::KeyUp(fn_key as u16));
                } else {
                    self.emit_dpad(code, false, actions);
                }
            }
            _ => {
                if self.mode == WorkMode::Gamepad {
                    actions.push(Action::KeyUp(code as u16));
                    return;
                }
                if self.mode == WorkMode::Keyboard {
                    self.on_keyboard_button_up(code, actions);
                } else if self.mode == WorkMode::Mouse {
                    self.on_mouse_button_up(code, actions);
                }
            }
        }
    }

    fn on_axis(&mut self, axis: u32, value: f32) {
        match axis {
            2 => self.left_joy.0 = value,
            3 => self.left_joy.1 = value,
            4 => self.right_joy.0 = value,
            5 => self.right_joy.1 = value,
            _ => {}
        }
    }

    /// 鼠标模式：根据摇杆当前位置持续输出鼠标移动
    fn emit_mouse_motion(&mut self, actions: &mut Vec<Action>) {
        const MOUSE_DEADZONE: f32 = 0.02;
        const SCROLL_DEADZONE: f32 = 0.08;
        /// 正常模式速度基准 12 px/帧 = 720 px/s
        const BASE_SPEED: f64 = 12.0;
        /// 精细模式最大速度 90 px/s，换算为 90/60 px/帧
        const FINE_SPEED: f64 = 90.0 / 60.0;
        /// 逐轴死区：人手推摇杆时另一轴的自然偏差可达 0.15~0.2，
        /// 低于此值的单轴分量视为非有意斜向，不产生移动
        /// 0.18 ≈ ±10°（tan(10°) = 0.176）
        const AXIS_DEADZONE: f64 = 0.18;

        // 左摇杆 → 鼠标指针（先做逐轴死区）
        let (lx, ly) = (self.left_joy.0 as f64, self.left_joy.1 as f64);
        let fx = if lx.abs() < AXIS_DEADZONE { 0.0 } else { lx };
        let fy = if ly.abs() < AXIS_DEADZONE { 0.0 } else { ly };
        let mag = (fx * fx + fy * fy).sqrt();

        if self.mouse.fine_mode() {
            // === 精细模式 ===
            if mag > MOUSE_DEADZONE as f64 {
                let nx = fx / mag;
                let ny = fy / mag;

                if !self.mouse_fine_primed {
                    self.mouse_fine_primed = true;
                    self.mouse_accum.0 += nx * 2.0;
                    self.mouse_accum.1 += ny * 2.0;
                }

                let factor = ((mag - MOUSE_DEADZONE as f64) / (1.0 - MOUSE_DEADZONE as f64)).clamp(0.0, 1.0);
                let speed = factor * FINE_SPEED;
                self.mouse_accum.0 += nx * speed;
                self.mouse_accum.1 += ny * speed;
            } else {
                self.mouse_fine_primed = false;
                self.mouse_accum.0 *= 0.8;
                self.mouse_accum.1 *= 0.8;
                if self.mouse_accum.0.abs() < 0.01 { self.mouse_accum.0 = 0.0; }
                if self.mouse_accum.1.abs() < 0.01 { self.mouse_accum.1 = 0.0; }
            }

            // 按累加矢量的模长触发，每次只在主轴方向发 1px
            let accum_mag = self.mouse_accum.0.hypot(self.mouse_accum.1);
            if accum_mag >= 1.0 {
                if self.mouse_accum.0.abs() >= self.mouse_accum.1.abs() {
                    let step = self.mouse_accum.0.signum() as i32;
                    actions.push(Action::MouseMove { dx: step as f64, dy: 0.0 });
                    self.mouse_accum.0 -= step as f64;
                } else {
                    let step = self.mouse_accum.1.signum() as i32;
                    actions.push(Action::MouseMove { dx: 0.0, dy: step as f64 });
                    self.mouse_accum.1 -= step as f64;
                }
            }
        } else {
            // === 正常模式 ===
            if mag > MOUSE_DEADZONE as f64 {
                let speed = (mag * mag) * BASE_SPEED;
                let dx = (fx / mag) * speed;
                let dy = (fy / mag) * speed;
                actions.push(Action::MouseMove { dx, dy });
            }
            // 退出精细模式时重置累加器
            self.mouse_accum = (0.0, 0.0);
            self.mouse_fine_primed = false;
        }

        // 右摇杆 → 滚轮：X=水平滚动，Y=垂直滚动
        let scroll_x = EventEngine::process_scroll_axis(self.right_joy.0, &mut self.scroll_accum.0, &mut self.scroll_primed.0);
        let scroll_y = EventEngine::process_scroll_axis(-self.right_joy.1, &mut self.scroll_accum.1, &mut self.scroll_primed.1);
        if scroll_x != 0 || scroll_y != 0 {
            actions.push(Action::MouseWheel { x: scroll_x, y: scroll_y });
        }
    }

    /// 处理单个轴向的滚轮累加
    /// 将摇杆幅度从 [deadzone, 1.0] 线性映射到 [min_rate, max_rate]
    /// 起手先滚一格（出死区即刻触发一次），然后进入累加模式
    fn process_scroll_axis(stick: f32, accum: &mut f32, primed: &mut bool) -> i32 {
        const SCROLL_DEADZONE: f32 = 0.08;
        const SCROLL_RATE_MIN: f32 = 1.0 / 60.0;   // ~0.0167/帧 (1 tick/sec)
        const SCROLL_RATE_MAX: f32 = 5.0 / 60.0;   // ~0.0833/帧 (5 ticks/sec)

        if stick.abs() > SCROLL_DEADZONE {
            if !*primed {
                *primed = true;
                let sign = if stick > 0.0 { 1 } else { -1 };
                return sign;  // 起手先送一格
            }
            let factor = (stick.abs() - SCROLL_DEADZONE) / (1.0 - SCROLL_DEADZONE);
            let rate = SCROLL_RATE_MIN + factor * (SCROLL_RATE_MAX - SCROLL_RATE_MIN);
            *accum += stick.signum() * rate;
        } else {
            *primed = false;
            *accum *= 0.5;
            if accum.abs() < 0.01 {
                *accum = 0.0;
            }
        }
        if accum.abs() >= 1.0 {
            let scroll = accum.trunc() as i32;
            *accum -= scroll as f32;
            return scroll;
        }
        0
    }

    /// FN 键松开：根据按压时长决定 tap/hold
    fn on_function_up(&mut self, actions: &mut Vec<Action>) {
        let elapsed = self.fn_pressed_at
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        self.fn_pressed_at = None;

        if self.mode == WorkMode::Gamepad {
            // 手柄模式松开 FN → 结束透传
            actions.push(Action::KeyUp(BTN_FN as u16));
            // 判断：长按 → 回到上一个非手柄模式
            if elapsed >= self.hold_threshold_ms {
                self.mode = self.prev_mode;
            }
            return;
        }

        if elapsed < self.hold_threshold_ms {
            // 短按：键盘 ↔ 鼠标
            self.mode = match self.mode {
                WorkMode::Keyboard => WorkMode::Mouse,
                WorkMode::Mouse => WorkMode::Keyboard,
                _ => WorkMode::Keyboard,
            };
        } else {
            // 长按：切到手柄模式（保存当前模式）
            self.prev_mode = self.mode;
            self.mode = WorkMode::Gamepad;
        }
    }

    fn on_l1_down(&mut self) {
        self.layer.fn_down();
        // L1 也触发鼠标精细模式（不论 FN lock 状态）
        if self.mode == WorkMode::Mouse {
            self.fine_l1_active = true;
            self.fine_exit_at = None;
            self.mouse.set_fine_mode(true);
        }
    }
    fn on_l1_up(&mut self) {
        self.layer.fn_up();
        // L1 松开立即退出精细模式（无延迟）
        if self.mode == WorkMode::Mouse {
            self.fine_l1_active = false;
            self.mouse.set_fine_mode(false);
            self.mouse_accum = (0.0, 0.0);
            self.mouse_fine_primed = false;
        }
    }

    fn on_keyboard_button_down(&mut self, code: u32, actions: &mut Vec<Action>) {
        // 先尝试直接按键映射
        if let Some(key) = self.direct_key_map(code, true) {
            actions.push(key);
            return;
        }

        // 再尝试网格映射（L3/R3 触发的格子）
        let layer = self.layer.current();
        let layout = match layer {
            Layer::Base => &BASE_LAYOUT,
            Layer::Fn => &FN_LAYOUT,
        };
        if let Some(cell) = self.map_button_to_grid(code) {
            if let Some(key_name) = get_key_name(layout, cell, false) {
                if let Some((key_code, _mods)) = parse_key_name(key_name) {
                    actions.push(Action::KeyDown(key_code));
                }
            }
        }
    }

    fn on_keyboard_button_up(&mut self, code: u32, actions: &mut Vec<Action>) {
        if let Some(key) = self.direct_key_map(code, false) {
            actions.push(key);
            return;
        }
    }

    /// 物理按键 → 键盘按键直接映射（根据设计文档 5.1 键盘模式按键映射表）
    fn direct_key_map(&self, code: u32, is_down: bool) -> Option<Action> {
        let key = match code {
            BTN_A => 28,   // Enter
            BTN_B => 1,    // Esc
            BTN_X => 111,  // Delete
            BTN_Y => 110,  // Insert
            BTN_L1 => return None, // FN 键，已单独处理
            BTN_L2 => 29,  // Ctrl
            BTN_R1 => 14,  // Backspace
            BTN_R2 => 56,  // Alt
            BTN_L3 => return None, // 网格触发键
            BTN_R3 => return None,
            BTN_SE => return None, // SE/ST 状态机处理
            BTN_ST => return None,
            BTN_FN => 312, // Menu 键透传（keytest 可见）
            _ => return None,
        };
        Some(if is_down { Action::KeyDown(key) } else { Action::KeyUp(key) })
    }

    fn on_mouse_button_down(&mut self, code: u32, actions: &mut Vec<Action>) {
        match code {
            BTN_X => { actions.push(Action::KeyDown(273)); } // BTN_RIGHT
            BTN_Y => { actions.push(Action::KeyDown(272)); } // BTN_LEFT
            BTN_R1 => {
                // FN 层激活时 R1 → Delete，否则 Backspace
                let key = if self.layer.current() == Layer::Fn { 111 } else { 14 };
                actions.push(Action::KeyDown(key));
            }
            _ => {
                if let Some(key) = self.direct_key_map(code, true) {
                    actions.push(key);
                }
            }
        }
    }

    fn on_mouse_button_up(&mut self, code: u32, actions: &mut Vec<Action>) {
        match code {
            BTN_X => { actions.push(Action::KeyUp(273)); }
            BTN_Y => { actions.push(Action::KeyUp(272)); }
            BTN_R1 => {
                let key = if self.layer.current() == Layer::Fn { 111 } else { 14 };
                actions.push(Action::KeyUp(key));
            }
            _ => {
                if let Some(key) = self.direct_key_map(code, false) {
                    actions.push(key);
                }
            }
        }
    }

    /// D-pad 方向键：任何模式下透传
    fn emit_dpad(&self, code: u32, is_down: bool, actions: &mut Vec<Action>) {
        if is_down {
            actions.push(Action::KeyDown(code as u16));
        } else {
            actions.push(Action::KeyUp(code as u16));
        }
    }

    fn map_button_to_grid(&self, code: u32) -> Option<usize> {
        match code {
            BTN_L3 => self.left_cell_locked,
            BTN_R3 => self.right_cell_locked,
            _ => None,
        }
    }

    fn actions_from_se_st(&self, se_actions: Vec<SeStAction>, actions: &mut Vec<Action>) {
        for action in se_actions {
            match action {
                SeStAction::KeyDown(code) => actions.push(Action::KeyDown(code)),
                SeStAction::KeyUp(code) => actions.push(Action::KeyUp(code)),
                SeStAction::ShiftDown(code) => actions.push(Action::KeyDown(code)),
                SeStAction::ShiftUp(code) => actions.push(Action::KeyUp(code)),
            }
        }
    }

    /// 返回完整状态转储字符串（调试用）
    pub fn state_dump(&self) -> String {
        let mode_str = match self.mode {
            WorkMode::Keyboard => "KEYBOARD",
            WorkMode::Mouse => "MOUSE",
            WorkMode::Gamepad => "GAMEPAD",
        };
        let layer_str = match self.layer.current() {
            Layer::Base => "Base",
            Layer::Fn => "FN",
        };
        let fn_lock = if self.layer.is_fn_locked() { " 🔒" } else { "" };

        let y = |s: &str| format!("\x1b[33m{}\x1b[0m", s); // ANSI yellow

        let fn_elapsed = self.fn_pressed_at
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let fn_state = if self.fn_pressed_at.is_some() {
            if fn_elapsed >= self.hold_threshold_ms { "HOLD" } else { "DOWN" }
        } else { "IDLE" };

        let mut s = String::new();
        s.push_str(&format!("Mode: {}  Layer: {}  FN: {}{}\n", y(mode_str), y(layer_str), y(fn_state), fn_lock));

        // 按键角色
        s.push_str("BTN: A→Enter B→Esc X→Delete Y→Insert");
        s.push_str("  L1→FN L2→Ctrl R1→BS R2→Alt");
        s.push_str(&format!("  SE→Tab/Shift({}) ST→Space/Shift({})\n",
            y(match self.se_st.se.state() {
                SeStState::Idle => "IDLE", SeStState::Waiting => "WAIT",
                SeStState::ShiftDown => "SHIFT", SeStState::KeyDown => "KEYDOWN",
            }),
            y(match self.se_st.st.state() {
                SeStState::Idle => "IDLE", SeStState::Waiting => "WAIT",
                SeStState::ShiftDown => "SHIFT", SeStState::KeyDown => "KEYDOWN",
            })));

        if self.mode == WorkMode::Mouse {
            let fine = if self.mouse.fine_mode() { "FINE" } else { "NORM" };
            s.push_str(&format!("MOUSE: X→R-btn Y→L-btn R3→M-btn  Sensitivity: {}  Mode: {}\n",
                y(&format!("{:.1}", self.mouse.sensitivity())), y(fine)));
        }

        // D-pad
        s.push_str("DPAD: ↑D-pad ↓D-pad ←D-pad →D-pad\n");

        // 摇杆 + 网格
        let l_cell = self.left_cell_locked.or_else(|| self.left_grid.selected_cell(self.left_joy));
        let r_cell = self.right_cell_locked.or_else(|| self.right_grid.selected_cell(self.right_joy));
        let l_cell_str = l_cell.map(|c| format!("grid{}", c)).unwrap_or_else(|| "--".into());
        let r_cell_str = r_cell.map(|c| format!("grid{}", c)).unwrap_or_else(|| "--".into());
        s.push_str(&format!("LS: ({}, {}) [{}]  RS: ({}, {}) [{}]\n",
            y(&format!("{:+.2}", self.left_joy.0)),
            y(&format!("{:+.2}", self.left_joy.1)),
            y(&l_cell_str),
            y(&format!("{:+.2}", self.right_joy.0)),
            y(&format!("{:+.2}", self.right_joy.1)),
            y(&r_cell_str)));
        s.push_str("---\n");
        s
    }
}

fn get_key_name<'a>(layout: &'a [[&'a str; 10]; 3], cell: usize, right_grid: bool) -> Option<&'a str> {
    let row = cell / 5;
    let col = (cell % 5) + if right_grid { 5 } else { 0 };
    if row < 3 && col < 10 {
        Some(layout[row][col])
    } else {
        None
    }
}
