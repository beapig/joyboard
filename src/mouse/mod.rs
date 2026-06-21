/// 鼠标模式逻辑
pub struct MouseEngine {
    sensitivity: f32,
    fine_dpi_scale: f32,
    fine_mode: bool,
}

impl MouseEngine {
    pub fn new(sensitivity: f32, fine_dpi_scale: f32) -> Self {
        Self {
            sensitivity,
            fine_dpi_scale,
            fine_mode: false,
        }
    }

    pub fn fine_mode(&self) -> bool { self.fine_mode }
    pub fn sensitivity(&self) -> f32 { self.sensitivity }

    /// 设置精细模式
    pub fn set_fine_mode(&mut self, enabled: bool) {
        self.fine_mode = enabled;
    }

    /// 摇杆输入 → 鼠标位移
    pub fn process_joystick(&self, x: f32, y: f32) -> (f64, f64) {
        let scale = if self.fine_mode {
            self.sensitivity * self.fine_dpi_scale
        } else {
            self.sensitivity
        };
        (x as f64 * scale as f64, y as f64 * scale as f64)
    }
}
