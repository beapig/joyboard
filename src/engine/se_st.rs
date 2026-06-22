use crate::config::keymap::keycode_from_name;
use std::time::Instant;

/// SE/ST 状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeStState {
    Idle,
    Waiting,   // 等待 hold 判定（时间信息在 SeStSM 中维护）
    ShiftDown,
    KeyDown,
}

/// SE/ST 独立状态机
#[derive(Debug, Clone)]
pub struct SeStSM {
    state: SeStState,
    pressed_at: Option<Instant>,
    /// 自己的键值码 (Tab / Space)
    own_key: u16,
    /// 自己的 Shift 码 (LeftShift / RightShift)
    own_shift: u16,
}

impl SeStSM {
    pub fn new(own_key: u16, own_shift: u16) -> Self {
        Self {
            state: SeStState::Idle,
            pressed_at: None,
            own_key,
            own_shift,
        }
    }

    pub fn state(&self) -> SeStState {
        self.state
    }

    pub fn own_key(&self) -> u16 {
        self.own_key
    }

    pub fn own_shift(&self) -> u16 {
        self.own_shift
    }

    pub fn elapsed_since_press(&self) -> Option<u64> {
        self.pressed_at
            .map(|t| t.elapsed().as_millis() as u64)
    }

    /// 检查是否已超过 hold 阈值，若是则转移到 ShiftDown
    pub fn try_hold(&mut self, hold_threshold_ms: u64) -> Option<SeStAction> {
        if self.state == SeStState::Waiting {
            if let Some(elapsed) = self.elapsed_since_press() {
                if elapsed >= hold_threshold_ms {
                    self.state = SeStState::ShiftDown;
                    self.pressed_at = None;
                    return Some(SeStAction::ShiftDown(self.own_shift));
                }
            }
        }
        None
    }
}

/// SE/ST 对
#[derive(Debug)]
pub struct SeStPair {
    pub se: SeStSM,
    pub st: SeStSM,
    tap_threshold_ms: u64,
    hold_threshold_ms: u64,
    /// 第三方按键标志：检测 SE/ST 是否需要提前触发 Shift
    third_party_shifted_se: bool,
    third_party_shifted_st: bool,
}

/// 状态机输出动作
#[derive(Debug, Clone)]
pub enum SeStAction {
    KeyDown(u16),
    KeyUp(u16),
    ShiftDown(u16),
    ShiftUp(u16),
}

impl SeStPair {
    pub fn new(tap_threshold_ms: u64, hold_threshold_ms: u64) -> Self {
        let tab = keycode_from_name("tab").unwrap_or(15);
        let space = keycode_from_name("space").unwrap_or(57);
        let lshift = keycode_from_name("lshift").unwrap_or(42);
        let rshift = keycode_from_name("rshift").unwrap_or(54);
        Self {
            se: SeStSM::new(tab, lshift),
            st: SeStSM::new(space, rshift),
            tap_threshold_ms,
            hold_threshold_ms,
            third_party_shifted_se: false,
            third_party_shifted_st: false,
        }
    }

    pub fn reset_third_party_flags(&mut self) {
        self.third_party_shifted_se = false;
        self.third_party_shifted_st = false;
    }

    pub fn on_se_down(&mut self, now: Instant) -> Vec<SeStAction> {
        let mut actions = Vec::new();
        match self.st.state {
            SeStState::Idle => {
                self.se.state = SeStState::Waiting;
                self.se.pressed_at = Some(now);
            }
            SeStState::Waiting => {
                // ST 在 WAITING → 触发 RightShift + Tab
                actions.push(SeStAction::ShiftDown(self.st.own_shift));
                actions.push(SeStAction::KeyDown(self.se.own_key));
                self.st.state = SeStState::ShiftDown;
                self.se.state = SeStState::KeyDown;
                self.se.pressed_at = None;
            }
            SeStState::ShiftDown => {
                // ST 已 Shift → 只发 Tab
                actions.push(SeStAction::KeyDown(self.se.own_key));
                self.se.state = SeStState::KeyDown;
                self.se.pressed_at = None;
            }
            SeStState::KeyDown => {
                // ST 已在 Key 状态 → 回退 WAITING
                self.se.state = SeStState::Waiting;
                self.se.pressed_at = Some(now);
            }
        }
        actions
    }

    pub fn on_st_down(&mut self, now: Instant) -> Vec<SeStAction> {
        let mut actions = Vec::new();
        match self.se.state {
            SeStState::Idle => {
                self.st.state = SeStState::Waiting;
                self.st.pressed_at = Some(now);
            }
            SeStState::Waiting => {
                // SE 在 WAITING → 触发 LeftShift + Space
                actions.push(SeStAction::ShiftDown(self.se.own_shift));
                actions.push(SeStAction::KeyDown(self.st.own_key));
                self.se.state = SeStState::ShiftDown;
                self.st.state = SeStState::KeyDown;
                self.st.pressed_at = None;
            }
            SeStState::ShiftDown => {
                // SE 已 Shift → 只发 Space
                actions.push(SeStAction::KeyDown(self.st.own_key));
                self.st.state = SeStState::KeyDown;
                self.st.pressed_at = None;
            }
            SeStState::KeyDown => {
                // SE 已在 Key 状态 → 回退 WAITING
                self.st.state = SeStState::Waiting;
                self.st.pressed_at = Some(now);
            }
        }
        actions
    }

    pub fn on_se_up(&mut self) -> Vec<SeStAction> {
        let mut actions = Vec::new();
        match self.se.state {
            SeStState::Waiting => {
                // tap: key down + key up
                let elapsed = self.se.elapsed_since_press().unwrap_or(0);
                if elapsed < self.tap_threshold_ms {
                    actions.push(SeStAction::KeyDown(self.se.own_key));
                    actions.push(SeStAction::KeyUp(self.se.own_key));
                } else if !self.third_party_shifted_se {
                    // 超时但未被其他打断 → 触发 Shift
                    actions.push(SeStAction::ShiftDown(self.se.own_shift));
                    actions.push(SeStAction::ShiftUp(self.se.own_shift));
                }
            }
            SeStState::ShiftDown => {
                actions.push(SeStAction::ShiftUp(self.se.own_shift));
            }
            SeStState::KeyDown => {
                actions.push(SeStAction::KeyUp(self.se.own_key));
            }
            SeStState::Idle => {}
        }
        self.se.state = SeStState::Idle;
        self.se.pressed_at = None;
        actions
    }

    pub fn on_st_up(&mut self) -> Vec<SeStAction> {
        let mut actions = Vec::new();
        match self.st.state {
            SeStState::Waiting => {
                let elapsed = self.st.elapsed_since_press().unwrap_or(0);
                if elapsed < self.tap_threshold_ms {
                    actions.push(SeStAction::KeyDown(self.st.own_key));
                    actions.push(SeStAction::KeyUp(self.st.own_key));
                } else if !self.third_party_shifted_st {
                    actions.push(SeStAction::ShiftDown(self.st.own_shift));
                    actions.push(SeStAction::ShiftUp(self.st.own_shift));
                }
            }
            SeStState::ShiftDown => {
                actions.push(SeStAction::ShiftUp(self.st.own_shift));
            }
            SeStState::KeyDown => {
                actions.push(SeStAction::KeyUp(self.st.own_key));
            }
            SeStState::Idle => {}
        }
        self.st.state = SeStState::Idle;
        self.st.pressed_at = None;
        actions
    }

    /// 第三方按键按下时检查 SE/ST 状态
    /// 如果任一处于 WAITING，提前触发 Shift
    pub fn on_third_party_down(&mut self) -> Vec<SeStAction> {
        let mut actions = Vec::new();
        if self.se.state == SeStState::Waiting {
            actions.push(SeStAction::ShiftDown(self.se.own_shift));
            self.third_party_shifted_se = true;
        }
        if self.st.state == SeStState::Waiting {
            actions.push(SeStAction::ShiftDown(self.st.own_shift));
            self.third_party_shifted_st = true;
        }
        actions
    }

    /// 每帧调用：检查 SE/ST 是否已超过 hold 阈值
    pub fn tick(&mut self) -> Vec<SeStAction> {
        let mut actions = Vec::new();
        if let Some(action) = self.se.try_hold(self.hold_threshold_ms) {
            actions.push(action);
        }
        if let Some(action) = self.st.try_hold(self.hold_threshold_ms) {
            actions.push(action);
        }
        actions
    }
}
