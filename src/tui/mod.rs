#[cfg(feature = "tui")]
pub mod tui_impl {
    use crate::engine::EngineState;

    pub struct Tui;

    impl Tui {
        pub fn new() -> std::io::Result<Self> {
            Ok(Self)
        }

        pub fn render(&mut self, _state: &EngineState) {
            // TODO: 实现 ratatui 终端界面
        }
    }
}

#[cfg(not(feature = "tui"))]
pub mod tui_impl {
    use crate::engine::EngineState;

    pub struct Tui;

    impl Tui {
        pub fn new() -> std::io::Result<Self> {
            Ok(Self)
        }

        pub fn render(&mut self, _state: &EngineState) {}
    }
}

pub use tui_impl::Tui;
