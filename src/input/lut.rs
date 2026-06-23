use crate::config::{Config, StickConfig};

/// 预计算的查找表
pub struct LutTable {
    axes: Vec<[f32; 65536]>,
}

impl LutTable {
    /// 预计算所有轴的 LUT
    /// axes 0-1 = 左摇杆 (axis 2,3)，axes 2-3 = 右摇杆 (axis 4,5)
    pub fn precompute(config: &Config) -> Self {
        let range_max = config.joystick.range_max;
        let mut axes = Vec::with_capacity(4);

        for side in [&config.joystick.left, &config.joystick.right] {
            let mut table = [0.0f32; 65536];
            for raw in i16::MIN..=i16::MAX {
                let idx = raw as u16;
                let abs = raw.unsigned_abs() as f32;
                // 使用硬件实际最大范围归一化，超过 max 的钳位为 1.0
                let normalized = raw.signum() as f32 * (abs / range_max).min(1.0);
                let with_deadzone = apply_deadzone(normalized, side);
                let curved = apply_curve(with_deadzone, side);
                table[idx as usize] = curved;
            }
            // Two axes (X, Y) share the same per-side config
            axes.push(table.clone());
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

fn apply_deadzone(value: f32, config: &StickConfig) -> f32 {
    let abs = value.abs();
    let center = config.deadzone.center;
    let edge = config.deadzone.edge;

    if abs < center {
        0.0
    } else if abs > edge {
        value.signum() * 1.0
    } else {
        let mapped = (abs - center) / (edge - center);
        value.signum() * mapped
    }
}

fn apply_curve(value: f32, config: &StickConfig) -> f32 {
    let abs = value.abs();
    let curved = match config.curve.curve_type.as_str() {
        "linear" => abs,
        "quadratic" => abs * abs,
        "cubic" => abs * abs * abs,
        _ => abs.powf(config.curve.power),
    };
    value.signum() * curved
}
