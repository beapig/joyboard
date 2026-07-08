/// 输出动作
#[derive(Debug, Clone)]
pub enum Action {
    KeyDown(u16),
    KeyUp(u16),
    MouseMove { dx: f64, dy: f64 },
    MouseWheel { x: i32, y: i32 },
}

/// 输出层抽象
pub trait OutputBackend {
    fn emit(&mut self, actions: &[Action]) -> std::io::Result<()>;
}

pub mod uinput;
