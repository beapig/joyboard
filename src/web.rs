/// Web 配置面板服务
///
/// 提供：
/// - 静态文件服务（web/index.html）
/// - GET /api/config — 读取当前配置
/// - POST /api/config — 写入配置并重建校准缓存
/// - WS /ws — 实时摇杆数据（raw + calibrated + grid cell）

use crate::config::Config;
use crate::engine::grid::JoystickGrid;
use crate::input::lut::LutTable;
use crate::input::{evdev::EvdevBackend, InputBackend, RawGamepadEvent};
use axum::{
    Router,
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

/// 校准缓存：保存时重建，evdev 循环每帧读
pub struct CalibrationCache {
    pub lut: LutTable,
    pub left_grid: JoystickGrid,
    pub right_grid: JoystickGrid,
}

impl CalibrationCache {
    fn new(cfg: &Config) -> Self {
        Self {
            lut: LutTable::precompute(cfg),
            left_grid: JoystickGrid::new(&cfg.joystick_grid.left),
            right_grid: JoystickGrid::new(&cfg.joystick_grid.right),
        }
    }
}

/// Web 服务共享状态
pub struct WebState {
    pub config: tokio::sync::RwLock<Config>,
    pub cal_cache: std::sync::RwLock<CalibrationCache>,
    pub joystick_tx: broadcast::Sender<String>,
}

/// 启动 Web 配置服务
pub async fn serve(port: u16, cfg: Config, evdev_path: Option<String>) {
    let (js_tx, _) = broadcast::channel(32);
    let evdev_tx = js_tx.clone();

    let state = Arc::new(WebState {
        cal_cache: std::sync::RwLock::new(CalibrationCache::new(&cfg)),
        config: tokio::sync::RwLock::new(cfg),
        joystick_tx: js_tx,
    });

    // 可选：读取 evdev 推送实时摇杆数据
    if let Some(path) = evdev_path {
        let state_clone = state.clone();
        tokio::task::spawn_blocking(move || {
            run_evdev_loop(&path, evdev_tx, state_clone);
        });
    }

    // 自动检测 web 资源目录
    let web_dir = if std::path::Path::new("web/index.html").exists() {
        "web".to_string()
    } else if std::path::Path::new("/usr/local/share/joyboard/web/index.html").exists() {
        "/usr/local/share/joyboard/web".to_string()
    } else {
        eprintln!("[WEB] 警告: 未找到 web/index.html");
        "web".to_string()
    };

    let app = Router::new()
        .route("/api/config", get(get_config).post(post_config))
        .route("/ws", get(ws_handler))
        .nest_service("/", ServeDir::new(&web_dir))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    eprintln!("[WEB] 配置面板: http://127.0.0.1:{}", port);
    eprintln!("[WEB] 局域网访问: http://<本机IP>:{}", port);
    eprintln!("[WEB] 按 Ctrl+C 停止服务");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// 读取 evdev 并广播摇杆状态（阻塞任务）
fn run_evdev_loop(path: &str, tx: broadcast::Sender<String>, state: Arc<WebState>) {
    let mut backend = match EvdevBackend::new(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[WEB] 无法打开 evdev {path}: {e}");
            return;
        }
    };
    eprintln!("[WEB] 已连接 evdev: {path}");

    let mut raw_left = (0.0f32, 0.0f32);
    let mut raw_right = (0.0f32, 0.0f32);
    let mut pressed_buttons: Vec<u32> = Vec::new();

    loop {
        match backend.poll() {
            Ok(events) => {
                if events.is_empty() {
                    std::thread::sleep(std::time::Duration::from_millis(16));
                    continue;
                }

                for event in &events {
                    match event {
                        RawGamepadEvent::Button { code, pressed } => {
                            if *pressed {
                                if !pressed_buttons.contains(code) {
                                    pressed_buttons.push(*code);
                                }
                            } else {
                                pressed_buttons.retain(|&c| c != *code);
                            }
                        }
                        RawGamepadEvent::Axis { axis, value } => {
                            let raw_val = *value as f32 / 4096.0;
                            let raw_val = raw_val.clamp(-1.0, 1.0);
                            match axis {
                                2 => raw_left.0 = raw_val,
                                3 => raw_left.1 = raw_val,
                                4 => raw_right.0 = raw_val,
                                5 => raw_right.1 = raw_val,
                                _ => {}
                            }
                        }
                    }
                }

                // 使用缓存校准（零内存分配）
                if let Ok(cache) = state.cal_cache.read() {
                    let raw_to_i16 = |v: f32| -> i16 { (v * 4096.0).clamp(-4096.0, 4096.0) as i16 };
                    let cal_left = (
                        cache.lut.lookup(0, raw_to_i16(raw_left.0)),
                        cache.lut.lookup(1, raw_to_i16(raw_left.1)),
                    );
                    let cal_right = (
                        cache.lut.lookup(2, raw_to_i16(raw_right.0)),
                        cache.lut.lookup(3, raw_to_i16(raw_right.1)),
                    );

                    let left_cell = cache.left_grid.selected_cell(cal_left);
                    let right_cell = cache.right_grid.selected_cell(cal_right);

                    let mut btn_map: HashMap<&str, u32> = HashMap::new();
                    for &code in &pressed_buttons {
                        let name = match code {
                            304 => "A", 305 => "B", 307 => "X", 306 => "Y",
                            308 => "L1", 314 => "L2", 313 => "L3",
                            309 => "R1", 315 => "R2", 316 => "R3",
                            310 => "SE", 311 => "ST", 312 => "FN",
                            115 => "VOL+", 114 => "VOL-",
                            _ => continue,
                        };
                        *btn_map.entry(name).or_insert(0) += 1;
                    }

                    let payload = serde_json::json!({
                        "type": "joystick",
                        "raw": {
                            "L": { "x": raw_left.0, "y": raw_left.1 },
                            "R": { "x": raw_right.0, "y": raw_right.1 },
                        },
                        "L": { "x": cal_left.0, "y": cal_left.1 },
                        "R": { "x": cal_right.0, "y": cal_right.1 },
                        "grid": { "L": left_cell, "R": right_cell },
                        "buttons": btn_map,
                    });

                    let _ = tx.send(payload.to_string());
                }
            }
            Err(e) => {
                eprintln!("[WEB] evdev 读取出错: {e}");
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
}

/// GET /api/config
async fn get_config(State(state): State<Arc<WebState>>) -> Json<Config> {
    let cfg = state.config.read().await;
    Json(cfg.clone())
}

/// POST /api/config — 保存配置并重建校准缓存
async fn post_config(
    State(state): State<Arc<WebState>>,
    Json(cfg): Json<Config>,
) -> Result<Json<Config>, StatusCode> {
    // 写入文件
    let path = crate::config::config_path();
    let toml_str = toml::to_string_pretty(&cfg).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(&path, &toml_str).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 更新内存配置
    let mut w = state.config.write().await;
    *w = cfg.clone();
    drop(w); // 尽快释放读锁

    // 重建校准缓存（LUT + Grids）
    let mut cache = state.cal_cache.write().unwrap();
    *cache = CalibrationCache::new(&cfg);

    eprintln!("[WEB] 配置已保存, LUT+Grids 已重建: {:?}", path);
    Ok(Json(cfg))
}

/// WS /ws — 实时摇杆数据推送
async fn ws_handler(
    State(state): State<Arc<WebState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<WebState>) {
    let mut rx = state.joystick_tx.subscribe();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(data) => {
                        if socket.send(Message::Text(data.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
