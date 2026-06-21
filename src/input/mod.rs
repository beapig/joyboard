use std::io;

/// 原始手柄事件
#[derive(Debug, Clone)]
pub enum RawGamepadEvent {
    Button { code: u32, pressed: bool },
    Axis { axis: u32, value: i16 },
}

/// 输入层抽象
pub trait InputBackend {
    fn poll(&mut self) -> io::Result<Vec<RawGamepadEvent>>;
}

/// 处理后的标准化事件
#[derive(Debug, Clone)]
pub enum GamepadEvent {
    ButtonDown { code: u32 },
    ButtonUp { code: u32 },
    AxisMotion { axis: u32, value: f32 },
}

/// 输入处理器：持有 LUT 和 HAT 状态，将硬件原始事件转换为逻辑事件
pub struct InputProcessor {
    lut: lut::LutTable,
    axis_map: Vec<u32>,
    /// D-pad HAT 左右前一帧值（-1/0/1）
    hat_x: i32,
    /// D-pad HAT 上下前一帧值（-1/0/1）
    hat_y: i32,
}

impl InputProcessor {
    pub fn new(lut: lut::LutTable, axis_map: Vec<u32>) -> Self {
        Self {
            lut,
            axis_map,
            hat_x: 0,
            hat_y: 0,
        }
    }

    /// 处理一批原始事件，输出逻辑事件
    /// 主要工作：
    ///   1. 按键事件直通
    ///   2. 摇杆轴经 LUT 查表
    ///   3. D-pad HAT 轴 → ButtonDown/ButtonUp（方向键）
    pub fn process(&mut self, raw: Vec<RawGamepadEvent>) -> Vec<GamepadEvent> {
        let mut out = Vec::new();

        for event in raw {
            match event {
                RawGamepadEvent::Button { code, pressed } => {
                    if pressed {
                        out.push(GamepadEvent::ButtonDown { code });
                    } else {
                        out.push(GamepadEvent::ButtonUp { code });
                    }
                }
                RawGamepadEvent::Axis { axis, value } => {
                    match axis {
                        // D-pad HAT → 按键事件
                        16 => self.process_hat_x(value as i32, &mut out),
                        17 => self.process_hat_y(value as i32, &mut out),
                        // 普通摇杆轴 → 经 LUT 后输出
                        _ => {
                            let lut_idx = self.axis_map.iter()
                                .position(|&a| a == axis)
                                .unwrap_or(0);
                            let processed = self.lut.lookup(lut_idx, value);
                            out.push(GamepadEvent::AxisMotion { axis, value: processed });
                        }
                    }
                }
            }
        }

        out
    }

    fn process_hat_x(&mut self, cur: i32, out: &mut Vec<GamepadEvent>) {
        let prev = self.hat_x;
        self.hat_x = cur;
        if prev == -1 { out.push(GamepadEvent::ButtonUp { code: 105 }); } // KEY_LEFT
        if prev ==  1 { out.push(GamepadEvent::ButtonUp { code: 106 }); } // KEY_RIGHT
        if cur == -1 { out.push(GamepadEvent::ButtonDown { code: 105 }); }
        if cur ==  1 { out.push(GamepadEvent::ButtonDown { code: 106 }); }
    }

    fn process_hat_y(&mut self, cur: i32, out: &mut Vec<GamepadEvent>) {
        let prev = self.hat_y;
        self.hat_y = cur;
        if prev == -1 { out.push(GamepadEvent::ButtonUp { code: 103 }); } // KEY_UP
        if prev ==  1 { out.push(GamepadEvent::ButtonUp { code: 108 }); } // KEY_DOWN
        if cur == -1 { out.push(GamepadEvent::ButtonDown { code: 103 }); }
        if cur ==  1 { out.push(GamepadEvent::ButtonDown { code: 108 }); }
    }
}

pub mod evdev;
pub mod lut;
