use crate::config::Config;

/// 预计算的查找表
pub struct LutTable {
    axes: Vec<[f32; 65536]>,
}

impl LutTable {
    /// 预计算所有轴的 LUT
    pub fn precompute(config: &Config) -> Self {
        let count = 4;
        let range_max = config.joystick.range_max;
        let mut axes = Vec::with_capacity(count);

        for _ in 0..count {
            let mut table = [0.0f32; 65536];
            for raw in i16::MIN..=i16::MAX {
                let idx = raw as u16;
                let abs = raw.unsigned_abs() as f32;
                // 使用硬件实际最大范围归一化，超过 max 的钳位为 1.0
                let normalized = raw.signum() as f32 * (abs / range_max).min(1.0);
                let with_deadzone = apply_deadzone(normalized, config);
                let curved = apply_curve(with_deadzone, config);
                table[idx as usize] = curved;
            }
            axes.push(table);
        }

        Self { axes }
    }

    /// 查表获取处理后的摇杆值
    #[inline]
    pub fn lookup(&self, axis: usize, raw: i16) -> f32 {
        if axis >= self.axes.len() {
            return 0.0;
        }
        self.axes[axis][raw as u16 as usize]
    }
}

fn apply_deadzone(value: f32, config: &Config) -> f32 {
    let abs = value.abs();
    let center = config.joystick.deadzone.center;
    let edge = config.joystick.deadzone.edge;

    if abs < center {
        0.0
    } else if abs > edge {
        value.signum() * 1.0
    } else {
        let mapped = (abs - center) / (edge - center);
        value.signum() * mapped
    }
}

fn apply_curve(value: f32, config: &Config) -> f32 {
    let abs = value.abs();
    let curved = match config.joystick.curve.curve_type.as_str() {
        "linear" => abs,
        "quadratic" => abs * abs,
        "cubic" => abs * abs * abs,
        _ => abs.powf(config.joystick.curve.power),
    };
    value.signum() * curved
}
