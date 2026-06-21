use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod keymap;

/// 所有运行时参数配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub evdev_path: String,
    pub log_level: String,
    pub joystick: JoystickConfig,
    pub mouse: MouseConfig,
    pub button_mode: ButtonModeConfig,
    pub joystick_grid: GridConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoystickConfig {
    pub deadzone: DeadzoneConfig,
    pub curve: CurveConfig,
    /// 硬件摇杆最大绝对值（ANBERNIC = 4096）
    pub range_max: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadzoneConfig {
    pub center: f32,
    pub edge: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveConfig {
    pub curve_type: String,
    pub power: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseConfig {
    pub sensitivity: f32,
    pub acceleration: bool,
    pub acceleration_curve: f32,
    pub fine_control: FineControlConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FineControlConfig {
    pub enable: bool,
    pub dpi_scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonModeConfig {
    pub tap_threshold_ms: u64,
    pub hold_threshold_ms: u64,
    pub extend_threshold_ms: u64,
    pub double_tap_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridConfig {
    pub left: GridSideConfig,
    pub right: GridSideConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridSideConfig {
    pub vertices: Vec<[f32; 2]>,
}

/// 校准数据，运行时保存到本地文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationData {
    pub axis_center: [i16; 4],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            evdev_path: "/dev/input/event1".into(),
            log_level: "info".into(),
            joystick: JoystickConfig {
                deadzone: DeadzoneConfig {
                    center: 0.15,
                    edge: 0.95,
                },
                curve: CurveConfig {
                    curve_type: "quadratic".into(),
                    power: 2.0,
                },
                range_max: 4096.0,
            },
            mouse: MouseConfig {
                sensitivity: 1.0,
                acceleration: true,
                acceleration_curve: 1.5,
                fine_control: FineControlConfig {
                    enable: true,
                    dpi_scale: 0.25,
                },
            },
            button_mode: ButtonModeConfig {
                tap_threshold_ms: 180,
                hold_threshold_ms: 400,
                extend_threshold_ms: 1200,
                double_tap_interval_ms: 300,
            },
            joystick_grid: default_grid_config(),
        }
    }
}

fn default_grid_config() -> GridConfig {
    let vertices = vec![
        [-1.0, -1.0], [-0.6, -1.0], [-0.2, -1.0],
        [0.2, -1.0],  [0.6, -1.0],  [1.0, -1.0],
        [-1.0, -0.33], [-0.6, -0.33], [-0.2, -0.33],
        [0.2, -0.33],  [0.6, -0.33],  [1.0, -0.33],
        [-1.0, 0.33],  [-0.6, 0.33],  [-0.2, 0.33],
        [0.2, 0.33],   [0.6, 0.33],   [1.0, 0.33],
        [-1.0, 1.0],   [-0.6, 1.0],   [-0.2, 1.0],
        [0.2, 1.0],    [0.6, 1.0],    [1.0, 1.0],
    ];
    GridConfig {
        left: GridSideConfig { vertices: vertices.clone() },
        right: GridSideConfig { vertices },
    }
}

impl Config {
    /// 加载配置：项目级 > 用户级 > 内置默认值
    pub fn load() -> Self {
        let path = config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            let cfg = Self::default();
            // 自动创建默认配置文件
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(content) = toml::to_string_pretty(&cfg) {
                let _ = std::fs::write(&path, content);
            }
            cfg
        }
    }
}

fn config_path() -> PathBuf {
    dirs_config_dir().join("joyboard").join("config.toml")
}

fn dirs_config_dir() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".config")
        })
}
