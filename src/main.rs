use std::io;

mod config;
mod engine;
mod input;
mod mouse;
mod output;
mod tui;

use crate::input::InputBackend;
use crate::output::OutputBackend;

fn print_help(program: &str) {
    eprintln!("JoyBoard — Linux 手柄映射键鼠");
    eprintln!();
    eprintln!("用法:");
    eprintln!("  {program}                         正常启动（读取配置中的设备）");
    eprintln!("  {program} evtest <设备路径>        调试: 打印逻辑摇杆/按键事件");
    eprintln!("  {program} keytest <设备路径>       调试: 打印引擎输出的键盘事件");
    eprintln!("  {program} debug <设备路径>         调试: 多阶段管线 + 状态面板");
    eprintln!("  {program} --help                   显示帮助");
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(|s| s.as_str()).unwrap_or("joyboard");

    if args.len() >= 2 && args[1] == "--help" {
        print_help(program);
        return Ok(());
    }

    if args.len() >= 3 && args[1] == "evtest" {
        return run_evtest(&args[2]);
    }

    if args.len() >= 3 && args[1] == "keytest" {
        return run_keytest(&args[2]);
    }

    if args.len() >= 3 && args[1] == "debug" {
        return run_debug(&args[2]);
    }

    run_normal()
}

/// 物理按键码 → 按键名
fn btn_name(code: u32) -> &'static str {
    match code {
        103 => "D-pad↑", 108 => "D-pad↓",
        105 => "D-pad←", 106 => "D-pad→",
        114 => "VOL_DOWN", 115 => "VOL_UP",
        304 => "A", 305 => "B", 306 => "Y", 307 => "X",
        308 => "L1", 309 => "R1", 310 => "SE", 311 => "ST",
        312 => "Menu", 313 => "L3", 314 => "L2", 315 => "R2",
        316 => "R3",
        _ => "?",
    }
}

/// 引擎动作 KeyCode → 可读名
fn action_name(code: u16) -> &'static str {
    key_name(code)
}

/// 调试模式：只输出输入层处理后的逻辑事件（不含引擎映射）
fn run_evtest(path: &str) -> io::Result<()> {
    let config = config::Config::load();
    let lut = input::lut::LutTable::precompute(&config);
    let mut backend = input::evdev::EvdevBackend::new(path)?;
    let mut proc = input::InputProcessor::new(lut, vec![2, 3, 4, 5]);

    eprintln!("监听设备: {path}");
    eprintln!("按下 Ctrl+C 退出");
    eprintln!("---");

    loop {
        let raw_events = match backend.poll() {
            Ok(events) => events,
            Err(e) => {
                eprintln!("读取失败: {e}");
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
        };

        if raw_events.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        let events = proc.process(raw_events);

        for event in &events {
            match event {
                input::GamepadEvent::ButtonDown { code } => {
                    println!("BTN code={code:>3} ({}) DOWN", evdev_name(*code));
                }
                input::GamepadEvent::ButtonUp { code } => {
                    println!("BTN code={code:>3} ({}) UP", evdev_name(*code));
                }
                input::GamepadEvent::AxisMotion { axis, value } => {
                    println!("AXIS axis={:>2} ({})  value={:+.3}", axis, axis_name(*axis), *value);
                }
            }
        }
    }
}

/// 调试模式：经过完整管线后打印引擎输出的键盘事件
fn run_keytest(path: &str) -> io::Result<()> {
    let config = config::Config::load();
    let lut = input::lut::LutTable::precompute(&config);
    let mut backend = input::evdev::EvdevBackend::new(path)?;
    let mut proc = input::InputProcessor::new(lut, vec![2, 3, 4, 5]);
    let mut engine = engine::EventEngine::new(&config);

    eprintln!("监听设备: {path}");
    eprintln!("按下 Ctrl+C 退出");
    eprintln!("---");

    loop {
        let raw_events = match backend.poll() {
            Ok(events) => events,
            Err(e) => {
                eprintln!("读取失败: {e}");
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
        };

        if raw_events.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        let events = proc.process(raw_events);
        let actions = engine.feed(events);

        for action in &actions {
            match *action {
                output::Action::KeyDown(code) => {
                    println!("KEY code={code:>3} ({}) DOWN", key_name(code));
                }
                output::Action::KeyUp(code) => {
                    println!("KEY code={code:>3} ({}) UP", key_name(code));
                }
                _ => {}
            }
        }
    }
}

/// 调试模式：完整管线 + 状态面板（不输出到 uinput）
fn run_debug(path: &str) -> io::Result<()> {
    let config = config::Config::load();
    let lut = input::lut::LutTable::precompute(&config);
    let mut backend = input::evdev::EvdevBackend::new(path)?;
    let mut proc = input::InputProcessor::new(lut, vec![2, 3, 4, 5]);
    let mut engine = engine::EventEngine::new(&config);

    eprintln!("监听设备: {path}");
    eprintln!("按下 Ctrl+C 退出");
    eprintln!("---");

    use std::time::Instant;

    let mut last_frame = Instant::now();
    const FRAME_DT: std::time::Duration = std::time::Duration::from_micros(16_666); // ~60 FPS

    loop {
        let raw_events = match backend.poll() {
            Ok(events) => events,
            Err(e) => {
                eprintln!("读取失败: {e}");
                std::thread::sleep(FRAME_DT);
                continue;
            }
        };

        let is_empty = raw_events.is_empty();

        // 固定 60 FPS：无论有无事件都走完整管线
        let events = if is_empty {
            Vec::new()
        } else {
            // [RAW] 原始事件
            for ev in &raw_events {
                match ev {
                    input::RawGamepadEvent::Button { code, pressed } => {
                        let state = if *pressed { "DOWN  " } else { "UP    " };
                        eprintln!("[RAW]  EV_KEY code={code:>3}({}) {}", btn_name(*code), state);
                    }
                    input::RawGamepadEvent::Axis { axis, value } => {
                        eprintln!("[RAW]  EV_ABS axis={axis:>2}({}) value={value:>6}", axis_name(*axis));
                    }
                }
            }
            let processed = proc.process(raw_events);

            // [PROC] 输入层处理后
            for event in &processed {
                match event {
                    input::GamepadEvent::ButtonDown { code } => {
                        eprintln!("[PROC] BTN down code={code:>3} ({})", btn_name(*code));
                    }
                    input::GamepadEvent::ButtonUp { code } => {
                        eprintln!("[PROC] BTN up   code={code:>3} ({})", btn_name(*code));
                    }
                    input::GamepadEvent::AxisMotion { axis, value } => {
                        eprintln!("[PROC] AXIS axis={:>2} ({})  value={:+.3}", axis, axis_name(*axis), *value);
                    }
                }
            }
            processed
        };

        // [ENGINE]
        let actions = engine.feed(events);
        let has_mm = actions.iter().any(|a| matches!(a, output::Action::MouseMove { .. }));

        for action in &actions {
            let stage = "[ENGINE]";
            match *action {
                output::Action::KeyDown(code) => {
                    eprintln!("{stage}  KEY {code:>3} ({}) DOWN", action_name(code));
                }
                output::Action::KeyUp(code) => {
                    eprintln!("{stage}  KEY {code:>3} ({}) UP", action_name(code));
                }
                output::Action::MouseMove { dx, dy } => {
                    eprintln!("{stage}  MOUSE  dx={dx:+.1} dy={dy:+.1}");
                }
                output::Action::MouseWheel { x, y } => {
                    eprintln!("{stage}  WHEEL  x={x:+} y={y:+}");
                }
            }
        }

        if is_empty && !has_mm {
            // 无事件且无鼠标移动时只显示状态面板
        } else {
            if !is_empty {
                eprintln!("[OUTPUT] [dry-run] uinput 未写入 (共 {} 个事件)", actions.len());
            }
            // 状态面板
            eprintln!("{}", engine.state_dump());
        }

        let elapsed = last_frame.elapsed();
        if elapsed < FRAME_DT {
            std::thread::sleep(FRAME_DT - elapsed);
        }
        last_frame = Instant::now();
    }
}

fn evdev_name(code: u32) -> &'static str {
    match code {
        103 => "D-pad Up", 108 => "D-pad Down",
        105 => "D-pad Left", 106 => "D-pad Right",
        114 => "VOL_DOWN", 115 => "VOL_UP",
        304 => "A", 305 => "B",
        306 => "Y", 307 => "X",
        308 => "L1", 309 => "R1",
        310 => "SE", 311 => "ST",
        312 => "Menu", 313 => "L3",
        314 => "L2", 315 => "R2",
        316 => "R3",
        _ => "?",
    }
}

fn axis_name(axis: u32) -> &'static str {
    match axis {
        2 => "LS X", 3 => "LS Y",
        4 => "RS X", 5 => "RS Y",
        _ => "?",
    }
}

fn key_name(code: u16) -> &'static str {
    let names: &[(u16, &str)] = &[
        (1, "ESC"), (2, "1"), (3, "2"), (4, "3"), (5, "4"),
        (6, "5"), (7, "6"), (8, "7"), (9, "8"), (10, "9"), (11, "0"),
        (12, "-"), (13, "="), (14, "BACKSPACE"), (15, "TAB"),
        (16, "Q"), (17, "W"), (18, "E"), (19, "R"), (20, "T"),
        (21, "Y"), (22, "U"), (23, "I"), (24, "O"), (25, "P"),
        (26, "["), (27, "]"), (28, "ENTER"),
        (29, "LCTRL"), (30, "A"), (31, "S"), (32, "D"), (33, "F"), (34, "G"),
        (35, "H"), (36, "J"), (37, "K"), (38, "L"),
        (39, ";"), (40, "'"), (41, "`"),
        (42, "LSHIFT"), (43, "\\"),
        (44, "Z"), (45, "X"), (46, "C"), (47, "V"), (48, "B"),
        (49, "N"), (50, "M"), (51, ","), (52, "."), (53, "/"),
        (54, "RSHIFT"), (56, "LALT"), (57, "SPACE"), (58, "CAPSLOCK"),
        (59, "F1"), (60, "F2"), (61, "F3"), (62, "F4"),
        (63, "F5"), (64, "F6"), (65, "F7"), (66, "F8"),
        (67, "F9"), (68, "F10"), (87, "F11"), (88, "F12"),
        (97, "RCTRL"), (100, "RALT"),
        (103, "UP"), (108, "DOWN"), (105, "LEFT"), (106, "RIGHT"),
        (102, "HOME"), (107, "END"), (104, "PAGEUP"), (109, "PAGEDOWN"),
        (110, "INSERT"), (111, "DELETE"),
        (114, "VOL_DOWN"), (115, "VOL_UP"),
        (272, "BTN_LEFT"), (273, "BTN_RIGHT"), (274, "BTN_MIDDLE"),
    ];
    names.iter().find(|(c, _)| *c == code).map(|(_, n)| *n).unwrap_or("?")
}

/// 正常启动模式
fn run_normal() -> io::Result<()> {
    let config = config::Config::load();

    match config.log_level.as_str() {
        "trace" => std::env::set_var("RUST_LOG", "trace"),
        "debug" => std::env::set_var("RUST_LOG", "debug"),
        "warn" => std::env::set_var("RUST_LOG", "warn"),
        "error" => std::env::set_var("RUST_LOG", "error"),
        _ => std::env::set_var("RUST_LOG", "info"),
    }
    env_logger::Builder::from_env(env_logger::Env::default())
        .target(env_logger::Target::Stderr)
        .init();

    let lut = input::lut::LutTable::precompute(&config);
    let mut proc = input::InputProcessor::new(lut, vec![2, 3, 4, 5]);

    let mut input_backend = input::evdev::EvdevBackend::new(&config.evdev_path)
        .map_err(|e| {
            log::error!("无法打开设备 {}: {}", config.evdev_path, e);
            e
        })?;

    let mut engine = engine::EventEngine::new(&config);

    #[cfg(target_os = "linux")]
    let mut output_backend = output::uinput::UInputBackend::new()?;

    let mut tui = tui::Tui::new()?;

    log::info!("JoyBoard 已启动");
    log::info!("  设备: {}", config.evdev_path);
    log::info!("  初始模式: {:?}", engine.mode);

    use std::time::Instant;

    let mut last_frame = Instant::now();
    const FRAME_DT: std::time::Duration = std::time::Duration::from_micros(16_666); // ~60 FPS

    loop {
        let raw_events = match input_backend.poll() {
            Ok(events) => events,
            Err(e) => {
                log::error!("读取输入事件失败: {}", e);
                std::thread::sleep(FRAME_DT);
                continue;
            }
        };

        let actions = if raw_events.is_empty() {
            engine.feed(Vec::new())
        } else {
            let events = proc.process(raw_events);
            engine.feed(events)
        };

        if !actions.is_empty() {
            #[cfg(target_os = "linux")]
            if let Err(e) = output_backend.emit(&actions) {
                log::error!("输出事件失败: {}", e);
            }
        }

        // 固定 60 FPS 帧率控制
        tui.render(&engine.state());
        let elapsed = last_frame.elapsed();
        if elapsed < FRAME_DT {
            std::thread::sleep(FRAME_DT - elapsed);
        }
        last_frame = Instant::now();
    }
}
