/// 两层切换逻辑
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layer {
    Base,
    Fn,
}

/// Layer 管理器
#[derive(Debug)]
pub struct LayerManager {
    current: Layer,
    fn_lock: bool,
}

impl LayerManager {
    pub fn new() -> Self {
        Self {
            current: Layer::Base,
            fn_lock: false,
        }
    }

    pub fn current(&self) -> Layer {
        self.current
    }

    /// FN 键按下（进入 FN 层）
    pub fn fn_down(&mut self) {
        if self.fn_lock {
            // FN Lock 状态下，短按退出 FN Lock
            self.fn_lock = false;
            self.current = Layer::Base;
        } else {
            self.current = Layer::Fn;
        }
    }

    /// FN 键松开（回到 Base 层）
    pub fn fn_up(&mut self) {
        if !self.fn_lock {
            self.current = Layer::Base;
        }
    }

    /// FN 双击 → 切换 FN Lock
    pub fn fn_double_tap(&mut self) {
        self.fn_lock = !self.fn_lock;
        if self.fn_lock {
            self.current = Layer::Fn;
        } else {
            self.current = Layer::Base;
        }
    }

    pub fn is_fn_locked(&self) -> bool {
        self.fn_lock
    }
}
