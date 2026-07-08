use joyboard_core::config::Config;
use joyboard_core::engine::grid::JoystickGrid;
use joyboard_core::engine::EventEngine;
use joyboard_core::input::lut::{apply_curve, apply_deadzone};
use joyboard_core::input::{evdev::EvdevBackend, InputBackend, RawGamepadEvent};
use joyboard_core::output::Action;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    http::StatusCode,
    response::Html,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

/// 状态共享
struct WebState {
    config: tokio::sync::RwLock<Config>,
    evdev_path: Option<String>,
    event_tx: broadcast::Sender<EngineSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
struct JoystickPos {
    x: f32,
    y: f32,
}

/// 推送给前端的完整快照
#[derive(Debug, Clone, Serialize)]
struct EngineSnapshot {
    #[serde(rename = "type")]
    msg_type: String,
    mode: String,
    layer: String,
    left_grid: [[&'static str; 5]; 3],
    right_grid: [[&'static str; 5]; 3],
    left_grid_selected: Option<usize>,
    right_grid_selected: Option<usize>,
    #[serde(rename = "L")]
    left_joystick: Option<JoystickPos>,
    #[serde(rename = "R")]
    right_joystick: Option<JoystickPos>,
    /// 原始摇杆值（未经过 deadzone/curve 校正）
    raw: Option<RawJoystickData>,
    #[serde(rename = "capslock_activated")]
    capslock: bool,
    buttons: std::collections::HashMap<String, bool>,
    actions: Vec<FrontendAction>,
}

#[derive(Debug, Clone, Serialize)]
struct RawJoystickData {
    #[serde(rename = "L")]
    left: Option<JoystickPos>,
    #[serde(rename = "R")]
    right: Option<JoystickPos>,
}

#[derive(Debug, Clone, Serialize)]
struct FrontendAction {
    t: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<i32>,
}

mod grid_name {
    use joyboard_core::config::keymap::{BASE_LAYOUT, FN_LAYOUT};

    pub fn layer_names(layer_idx: usize) -> [[&'static str; 5]; 3] {
        let l = if layer_idx == 0 { &BASE_LAYOUT } else { &FN_LAYOUT };
        let mut out: [[&'static str; 5]; 3] = [["?"; 5]; 3];
        for r in 0..3 {
            for c in 0..5 {
                out[r][c] = l[r][c];
            }
        }
        out
    }

    pub fn key_name(code: u16) -> &'static str {
        match code as u32 {
            1 => "ESC", 2 => "1", 3 => "2", 4 => "3", 5 => "4",
            6 => "5", 7 => "6", 8 => "7", 9 => "8", 10 => "9", 11 => "0",
            12 => "-", 13 => "=", 14 => "BS", 15 => "TAB",
            16 => "Q", 17 => "W", 18 => "E", 19 => "R", 20 => "T",
            21 => "Y", 22 => "U", 23 => "I", 24 => "O", 25 => "P",
            26 => "[", 27 => "]", 28 => "ENTER",
            29 => "CTRL", 30 => "A", 31 => "S", 32 => "D", 33 => "F", 34 => "G",
            35 => "H", 36 => "J", 37 => "K", 38 => "L",
            39 => ";", 40 => "'", 41 => "`",
            42 => "SHIFT", 43 => "\\",
            44 => "Z", 45 => "X", 46 => "C", 47 => "V", 48 => "B",
            49 => "N", 50 => "M", 51 => ",", 52 => ".", 53 => "/",
            54 => "RSHIFT", 56 => "LALT", 57 => "SPACE", 58 => "CAPS",
            59..=68 => ["F1","F2","F3","F4","F5","F6","F7","F8","F9","F10"][(code-59) as usize],
            87 => "F11", 88 => "F12",
            97 => "RCTRL", 100 => "RALT",
            103 => "UP", 108 => "DOWN", 105 => "LEFT", 106 => "RIGHT",
            102 => "HOME", 107 => "END", 104 => "PGUP", 109 => "PGDN",
            110 => "INS", 111 => "DEL",
            272 => "LCLICK", 273 => "RCLICK", 274 => "MCLICK",
            _ => "?",
        }
    }
}

/// 启动 Web 服务
pub async fn serve(port: u16, config: Config, evdev_path: Option<String>) {
    let (tx, _) = broadcast::channel::<EngineSnapshot>(16);

    let state = Arc::new(WebState {
        config: tokio::sync::RwLock::new(config),
        evdev_path,
        event_tx: tx,
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/api/config", get(get_config).put(put_config))
        .route("/ws", get(ws_handler))
        .nest_service("/web", ServeDir::new("web"))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    eprintln!("[WEB] 面板地址: http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> impl IntoResponse {
    Html(include_str!("../../web/index.html"))
}

async fn get_config(State(state): State<Arc<WebState>>) -> Json<Config> {
    let cfg = state.config.read().await;
    Json(cfg.clone())
}

async fn put_config(
    State(state): State<Arc<WebState>>,
    Json(new_cfg): Json<Config>,
) -> StatusCode {
    {
        let mut cfg = state.config.write().await;
        *cfg = new_cfg;
    }
    // 持久化：从 state 读当前值写入文件
    let final_cfg = state.config.read().await.clone();
    if let Ok(content) = toml::to_string_pretty(&final_cfg) {
        let path = joyboard_core::config::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&path, content).is_err() {
            log::error!("保存配置失败");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }
    StatusCode::OK
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut ws: WebSocket, state: Arc<WebState>) {
    let mut rx = state.event_tx.subscribe();

    // 启动事件引擎 + evdev 线程
    let evdev_path = state.evdev_path.clone();
    let cfg = state.config.read().await.clone();
    let event_tx = state.event_tx.clone();
    drop(state);

    let engine_handle = tokio::spawn(async move {
        run_engine_loop(evdev_path, cfg, event_tx).await;
    });

    // 不断发送快照给 WS 客户端
    loop {
        match rx.recv().await {
            Ok(snapshot) => {
                if let Ok(json) = serde_json::to_string(&snapshot) {
                    if ws.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }

    engine_handle.abort();
}

async fn run_engine_loop(
    evdev_path: Option<String>,
    initial_cfg: Config,
    event_tx: broadcast::Sender<EngineSnapshot>,
) {
    let mut input_backend: Option<EvdevBackend> = match &evdev_path {
        Some(path) if !path.is_empty() => match EvdevBackend::new(path) {
            Ok(b) => {
                log::info!("[WEB] evdev 设备已打开: {path}");
                Some(b)
            }
            Err(e) => {
                log::warn!("[WEB] 无法打开 evdev 设备 {path}: {e}");
                None
            }
        },
        _ => {
            log::warn!("[WEB] 未指定 evdev 路径，仅使用模拟数据");
            None
        }
    };

    let mut engine = EventEngine::new(&initial_cfg);
    let mut button_state: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    let mut frame_count = 0u64;

    let mut raw_left = (0.0f32, 0.0f32);
    let mut raw_right = (0.0f32, 0.0f32);

    loop {
        tokio::time::sleep(std::time::Duration::from_micros(16_666)).await;

        let raw_events: Vec<RawGamepadEvent> = if let Some(ref mut backend) = input_backend {
            match backend.poll() {
                Ok(events) => events,
                Err(e) => {
                    log::error!("读取 evdev 失败: {e}");
                    Vec::new()
                }
            }
        } else {
            generate_mock_events(&mut frame_count)
        };

        let cfg = Config::load();

        let mut processed_events = Vec::new();

        for event in raw_events {
            match event {
                RawGamepadEvent::Button { code, pressed } => {
                    if pressed {
                        processed_events.push(joyboard_core::input::GamepadEvent::ButtonDown { code });
                    } else {
                        processed_events.push(joyboard_core::input::GamepadEvent::ButtonUp { code });
                    }
                }
                RawGamepadEvent::Axis { axis, value } => {
                    match axis {
                        16 | 17 => {
                            let cur = value as i32;
                            if axis == 16 {
                                if cur == -1 { processed_events.push(joyboard_core::input::GamepadEvent::ButtonDown { code: 105 }); }
                                if cur ==  1 { processed_events.push(joyboard_core::input::GamepadEvent::ButtonDown { code: 106 }); }
                                if cur ==  0 {
                                    processed_events.push(joyboard_core::input::GamepadEvent::ButtonUp { code: 105 });
                                    processed_events.push(joyboard_core::input::GamepadEvent::ButtonUp { code: 106 });
                                }
                            } else {
                                if cur == -1 { processed_events.push(joyboard_core::input::GamepadEvent::ButtonDown { code: 103 }); }
                                if cur ==  1 { processed_events.push(joyboard_core::input::GamepadEvent::ButtonDown { code: 108 }); }
                                if cur ==  0 {
                                    processed_events.push(joyboard_core::input::GamepadEvent::ButtonUp { code: 103 });
                                    processed_events.push(joyboard_core::input::GamepadEvent::ButtonUp { code: 108 });
                                }
                            }
                        }
                        _ => {
                            let abs = value.unsigned_abs() as f32;
                            let normalized_raw = value.signum() as f32 * (abs / cfg.joystick.range_max).min(1.0);

                            if axis == 2 { raw_left.0 = normalized_raw; }
                            else if axis == 3 { raw_left.1 = normalized_raw; }
                            else if axis == 4 { raw_right.0 = normalized_raw; }
                            else if axis == 5 { raw_right.1 = normalized_raw; }

                            let config = match axis {
                                2 | 3 => &cfg.joystick.left,
                                4 | 5 => &cfg.joystick.right,
                                _ => &cfg.joystick.left,
                            };

                            let with_deadzone = apply_deadzone(normalized_raw, config);
                            let curved = apply_curve(with_deadzone, config);

                            processed_events.push(joyboard_core::input::GamepadEvent::AxisMotion { axis, value: curved });
                        }
                    }
                }
            }
        }

        let actions = engine.feed(processed_events);

        for a in &actions {
            match *a {
                Action::KeyDown(code) => {
                    let name = grid_name::key_name(code);
                    button_state.insert(name.to_string(), true);
                }
                Action::KeyUp(code) => {
                    let name = grid_name::key_name(code);
                    button_state.insert(name.to_string(), false);
                }
                _ => {}
            }
        }

        let frontend_actions: Vec<FrontendAction> = actions
            .into_iter()
            .filter_map(|a| match a {
                Action::KeyDown(code) => {
                    let name = grid_name::key_name(code).to_string();
                    Some(FrontendAction {
                        t: "kd",
                        n: Some(name),
                        dx: None,
                        dy: None,
                        x: None,
                        y: None,
                    })
                }
                Action::KeyUp(code) => {
                    let name = grid_name::key_name(code).to_string();
                    Some(FrontendAction {
                        t: "ku",
                        n: Some(name),
                        dx: None,
                        dy: None,
                        x: None,
                        y: None,
                    })
                }
                Action::MouseMove { dx, dy } => Some(FrontendAction {
                    t: "mm",
                    n: None,
                    dx: Some(dx),
                    dy: Some(dy),
                    x: None,
                    y: None,
                }),
                Action::MouseWheel { x, y } => Some(FrontendAction {
                    t: "mw",
                    n: None,
                    dx: None,
                    dy: None,
                    x: Some(x),
                    y: Some(y),
                }),
            })
            .collect();

        let st = engine.state();
        let layer_idx = match st.layer {
            joyboard_core::engine::layer::Layer::Fn => 1,
            _ => 0,
        };

        let snapshot = EngineSnapshot {
            msg_type: "joystick".to_string(),
            mode: format!("{:?}", st.mode),
            layer: format!("{:?}", st.layer),
            left_grid: grid_name::layer_names(layer_idx),
            right_grid: grid_name::layer_names(layer_idx),
            left_grid_selected: st.left_grid_selected,
            right_grid_selected: st.right_grid_selected,
            left_joystick: Some(JoystickPos { x: st.left_joystick.0, y: st.left_joystick.1 }),
            right_joystick: Some(JoystickPos { x: st.right_joystick.0, y: st.right_joystick.1 }),
            raw: Some(RawJoystickData {
                left: Some(JoystickPos { x: raw_left.0, y: raw_left.1 }),
                right: Some(JoystickPos { x: raw_right.0, y: raw_right.1 }),
            }),
            capslock: st.capslock_activated,
            buttons: button_state.clone(),
            actions: frontend_actions,
        };

        let _ = event_tx.send(snapshot);
    }
}

fn generate_mock_events(frame: &mut u64) -> Vec<RawGamepadEvent> {
    *frame += 1;
    let t = *frame as f64 * 0.016;
    let lx = (t * 0.7).sin();
    let ly = (t * 0.5).cos();
    let mut events = vec![
        RawGamepadEvent::Axis { axis: 2, value: (lx * 32767.0) as i16 },
        RawGamepadEvent::Axis { axis: 3, value: (ly * 32767.0) as i16 },
        RawGamepadEvent::Axis { axis: 4, value: ((t * 0.9).sin() * 32767.0) as i16 },
        RawGamepadEvent::Axis { axis: 5, value: ((t * 0.6).cos() * 32767.0) as i16 },
    ];
    // 模拟按钮
    if *frame % 120 == 0 {
        events.push(RawGamepadEvent::Button { code: 304, pressed: true });
    } else if *frame % 120 == 30 {
        events.push(RawGamepadEvent::Button { code: 304, pressed: false });
    }
    events
}
