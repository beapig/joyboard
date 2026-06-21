# JoyBoard — 技术架构设计

## 一、设计原则

| 原则             | 说明                                                                     |
| ---------------- | ------------------------------------------------------------------------ |
| **单文件二进制** | 编译为单个静态链接的 ELF 二进制，除 `/dev/uinput` 权限外无外部运行时依赖 |
| **分层解耦**     | 输入 → 事件引擎 → 输出 三层独立，通过 trait 接口通信                     |
| **无锁通信**     | 主循环 + 消息通道，不跨线程共享可变状态                                  |
| **零前端依赖**   | 配置 UI 为纯静态 HTML+JS，不引入 npm/node_modules                        |

---

## 二、整体架构

```
┌─────────────────────────────────────────────────────────┐
│                   JoyBoard 进程 (单线程 async)          │
│                                                         │
│  ┌──────────┐   ┌────────────┐   ┌──────────────────┐   │
│  │ 输入层   │   │ 事件引擎   │   │ 输出层           │   │
│  │ (evdev)  │──>│ (状态机)   │──>│ (uinput)         │   │
│  └──────────┘   └────────────┘   └──────────────────┘   │
│       │               │                                 │
│       │               │  ┌──────────────────┐           │
│       │               ├──│ TUI (ratatui)    │           │
│       │               │  └──────────────────┘           │
│       │               │  ┌──────────────────┐           │
│       │               └──│ Web Config UI    │           │
│       │                  │ (axum, optional) │           │
│       │                  └──────────────────┘           │
│       │                                                 │
│  ┌────┴──────┐                                          │
│  │  LUT 查表 │                                          │
│  │ (预计算)  │                                          │
│  └───────────┘                                          │
└─────────────────────────────────────────────────────────┘
```

### 线程模型

```
main thread (tokio async single-threaded):
  ├── [core] evdev poll (epoll / blocking_recv)
  ├── [core] 事件引擎 + 状态机
  ├── [core] uinput write
  ├── [tui]  terminal render (ratatui)
  └── [web]  axum HTTP (optional, spawned)
```

- **单线程 async 运行时**，整个核心流水线在同一个 tokio task 中串行执行
- 避免多线程带来的锁竞争和状态同步问题
- Web 配置服务通过 `tokio::spawn` 在同一个运行时中并发处理 HTTP 请求
- 跨线程通信仅限配置热重载（`Arc<RwLock<Config>>`）

---

## 三、项目结构

```
joyboard/
├── Cargo.toml
├── src/
│   ├── main.rs               # 入口：CLI 解析、启动核心循环
│   ├── config/
│   │   ├── mod.rs            # Config 结构体、默认值、TOML 解析
│   │   └── keymap.rs         # 写死的键位表 (Base/FN 层、网格键位)
│   ├── input/
│   │   ├── mod.rs            # InputBackend trait
│   │   ├── evdev.rs          # evdev 后端
│   │   └── lut.rs            # LUT 预计算 + 查表
│   ├── engine/
│   │   ├── mod.rs            # 事件循环、主状态机
│   │   ├── layer.rs          # Base/FN 层切换逻辑
│   │   ├── se_st.rs          # SE/ST 独立状态机
│   │   └── grid.rs           # 摇杆网格 (叉积判定)
│   ├── output/
│   │   ├── mod.rs            # OutputBackend trait
│   │   └── uinput.rs         # uinput 虚拟设备
│   ├── mouse/
│   │   └── mod.rs            # 鼠标模式逻辑
│   ├── tui/
│   │   └── mod.rs            # ratatui 终端界面
│   └── web/
│       ├── mod.rs            # axum HTTP server + router
│       └── static/           # 纯静态 HTML/JS/CSS
│           ├── index.html
│           ├── style.css
│           └── app.js
```

### 模块依赖关系

```
main.rs
  ├── config
  ├── input (→ lut)  
  ├── engine (→ layer, se_st, grid)
  ├── output
  ├── mouse
  ├── tui        (optional, Cargo feature)
  └── web        (optional, Cargo feature)
```

---

## 四、核心循环

```rust
// main.rs — 主循环伪代码
#[tokio::main]
async fn main() {
    let config = Config::load();
    let lut = LutTable::precompute(&config);
    let mut input = EvdevBackend::new(&config);
    let mut engine = EventEngine::new(&config, &lut);
    let mut output = UInputBackend::new();
    
    // 可选模块
    let mut tui = config.tui_enabled.then(|| Tui::new());
    
    // 启动 web server（可选）
    if config.web_enabled {
        tokio::spawn(web::serve(config.clone()));
    }
    
    // 主循环
    loop {
        // 1. 从 evdev 读取原始事件
        let raw_events = input.poll().await;
        
        // 2. 输入处理 (死区、LUT、按键消抖)
        let events = input.process(raw_events, &lut);
        
        // 3. 事件引擎状态机
        let actions = engine.feed(events);
        
        // 4. 输出到 uinput
        output.emit(&actions);
        
        // 5. TUI 刷新
        if let Some(ref tui) = tui { tui.render(&engine.state()); }
    }
}
```

### 事件流

```
evdev raw_event (struct input_event)
  │
  ▼
LUT 查表 (i16 → f32，摇杆轴)
  │
  ▼
EventEngine.feed()
  ├── button → button state machine (normal/tap-hold/double-tap)
  ├── axis → grid hit test → selected cell
  ├── SE/ST state machine
  └── FN lock toggle
  │
  ▼
Action 列表 [Action; N]
  │
  ▼
UInputBackend.emit(actions)
  ├── keyboard: write EV_KEY / EV_SYN
  └── mouse: write EV_REL / EV_SYN
```

---

## 五、输入层技术细节

### 5.1 evdev 后端

```rust
pub struct EvdevBackend {
    fd: RawFd,              // /dev/input/event* 的文件描述符
    axis_map: HashMap<u32, usize>,  // 轴号 → LUT 索引
    button_map: HashMap<u32, usize>, // 按键号 → 逻辑按键索引
}

impl InputBackend for EvdevBackend {
    fn poll(&mut self) -> io::Result<Vec<RawGamepadEvent>>;
}
```

- 使用非阻塞 I/O + `poll()` 或 `epoll()` 等待事件
- 避免忙轮询，降低 CPU 占用（Cortex-A53 上的关键考量）
- 读取事件后立即结构化，不做额外处理

### 5.2 LUT 预计算

LUT 仅在配置加载或热重载时计算一次，运行时只做整数索引查表：

```rust
pub struct LutTable {
    /// 每个轴一个 LUT，覆盖 i16 全范围 (-32768..32767)
    axes: [LookupTable; MAX_AXES],
}

struct LookupTable {
    table: [f32; 65536],  // 65536 = 2^16
}

impl LutTable {
    fn precompute(config: &Config) -> Self {
        for each axis {
            for raw in i16::MIN..=i16::MAX {
                // 归一化 → 死区裁剪 → 校准补偿 → 曲线整形
                let normalized = raw as f32 / i16::MAX as f32;
                let with_deadzone = apply_deadzone(normalized, config);
                let calibrated = apply_calibration(with_deadzone);
                let curved = apply_curve(calibrated, config);
                lut[raw as u16] = curved;
            }
        }
    }
    
    fn lookup(&self, axis: usize, raw: i16) -> f32 {
        self.axes[axis].table[raw as u16]
    }
}
```

- **LUT 大小**：每个轴 256KB（65536 × 4 bytes），4 个轴约 1MB
- **计算时机**：程序启动时、配置热重载时
- **运行时开销**：一次数组索引 + 一次 f32 读取，零浮点运算

### 5.3 校准持久化

```rust
pub struct CalibrationData {
    axis_center: [i16; 4],   // 每个轴的中心偏移
}
```

- 启动时读取 1 秒摇杆静止数据，取均值作为中心点
- 保存到 `~/.config/joyboard/calibration.json`
- 后续启动优先使用已保存的校准值，摇杆无响应时重新校准

---

## 六、事件引擎技术细节

### 6.1 按键状态机

```rust
#[derive(Clone, Copy, PartialEq)]
enum ButtonSM {
    Idle,
    Down { pressed_at: Instant },
    // Normal 模式
    NormalHold,
    // Extended tap-hold 模式
    TapPending { pressed_at: Instant },
    Holding { triggered: bool },
}

struct ButtonState {
    sm: ButtonSM,
    actions: ButtonActions,  // 从 keymap 解析
}
```

- 每个物理按键有一个 `ButtonState`
- 共享同一组 `tap_threshold_ms` / `hold_threshold_ms` 常量

### 6.2 SE/ST 独立状态机

```rust
#[derive(Clone, Copy, PartialEq)]
enum SeStState {
    Idle,
    Waiting { pressed_at: Instant },
    ShiftDown,      // Shift 已按下
    KeyDown,        // Tab/Space 已按下
}

struct SeStSM {
    state: SeStState,
    own_key: KeyCode,          // Tab 或 Space
    own_shift: KeyCode,        // LeftShift 或 RightShift
    peer: &SeStSM,             // 对方的引用（耦合，详见下方说明）
}
```

**关于互相引用**：SE 和 ST 状态机之间不需要持有引用。采用"查表法"——每个状态机在按下时通过一个共享的 `[SeStState; 2]` 数组查询对方状态即可。

```rust
pub struct SeStPair {
    se: SeStSM,
    st: SeStSM,
}

impl SeStPair {
    fn on_se_down(&mut self, now: Instant) -> Vec<Action> {
        match self.st.state {
            SeStState::Idle => { self.se.start_waiting(now); vec![] }
            SeStState::Waiting { .. } => {
                // ST 在 WAITING → 触发 RightShift + Tab
                self.st.state = SeStState::ShiftDown;
                self.se.state = SeStState::KeyDown;
                vec![Action::Key { code: KeyCode::RightShift, value: KeyValue::Pressed },
                     Action::Key { code: KeyCode::Tab, value: KeyValue::Pressed }]
            }
            SeStState::ShiftDown | SeStState::KeyDown => {
                // 对方已决策，按规则处理
                ...
            }
        }
    }
}
```

### 6.3 网格叉积判定

```rust
pub struct GridCell {
    // 四个顶点，按顺时针/逆时针顺序
    v0: (f32, f32),
    v1: (f32, f32),
    v2: (f32, f32),
    v3: (f32, f32),
}

impl GridCell {
    fn contains(&self, point: (f32, f32)) -> bool {
        fn cross(a: (f32, f32), b: (f32, f32)) -> f32 {
            a.0 * b.1 - a.1 * b.0
        }
        
        let ab = (self.v1.0 - self.v0.0, self.v1.1 - self.v0.1);
        let ap = (point.0 - self.v0.0, point.1 - self.v0.1);
        let bc = (self.v2.0 - self.v1.0, self.v2.1 - self.v1.1);
        let bp = (point.0 - self.v1.0, point.1 - self.v1.1);
        let cd = (self.v3.0 - self.v2.0, self.v3.1 - self.v2.1);
        let cp = (point.0 - self.v2.0, point.1 - self.v2.1);
        let da = (self.v0.0 - self.v3.0, self.v0.1 - self.v3.1);
        let dp = (point.0 - self.v3.0, point.1 - self.v3.1);
        
        let c1 = cross(ab, ap);
        let c2 = cross(bc, bp);
        let c3 = cross(cd, cp);
        let c4 = cross(da, dp);
        
        (c1 >= 0.0 && c2 >= 0.0 && c3 >= 0.0 && c4 >= 0.0)
            || (c1 <= 0.0 && c2 <= 0.0 && c3 <= 0.0 && c4 <= 0.0)
    }
}
```

- 每个摇杆 15 个格子，每帧最多遍历 15 次叉积判定
- 每帧一次 `grid_all_of_15` 不足 100 条浮点运算，Cortex-A53 上开销可忽略

---

## 七、输出层技术细节

### uinput 虚拟设备

```rust
pub struct UInputBackend {
    keyboard_fd: RawFd,  // /dev/uinput fd (虚拟键盘)
    mouse_fd: RawFd,     // /dev/uinput fd (虚拟鼠标)
}

impl UInputBackend {
    pub fn new() -> io::Result<Self> {
        // 1. open /dev/uinput
        // 2. UI_SET_EVBIT(EV_KEY), UI_SET_EVBIT(EV_REL)
        // 3. UI_SET_KEYBIT(KEY_*) for all supported keys
        // 4. UI_DEV_CREATE
    }
    
    pub fn emit(&self, events: &[InputEvent]) {
        /// 发送 EV_KEY / EV_REL / EV_SYN
        /// 
        /// write(fd, &event, size_of::<struct input_event>())
        /// 事件间插入 EV_SYN 分隔
    }
}
```

- **虚拟键盘**：注册全部 `KEY_*` 码
- **虚拟鼠标**：注册 `REL_X`、`REL_Y`、`REL_WHEEL`、`BTN_LEFT` ~ `BTN_SIDE`
- **权限**：需要 `CAP_SYS_ADMIN` 或 `udev` 规则
- **事件合并**：同一帧内多个相邻事件合并为单次 `write`（减少上下文切换）

### 输入事件结构

```rust
#[repr(C)]
struct input_event {
    time: timeval,
    type_: u16,     // EV_KEY / EV_REL / EV_SYN
    code: u16,      // KEY_A / REL_X / SYN_REPORT
    value: i32,     // 0=release / 1=press / 2=repeat / 位移量
}
```

---

## 八、TUI 技术方案

### 依赖

```toml
[dependencies]
ratatui = "0.26"
crossterm = "0.27"
```

### 实现

```rust
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    pub fn render(&mut self, state: &EngineState) {
        self.terminal.draw(|frame| {
            // 顶部：模式 + 当前 Layer
            // 中间：左右网格（15 个格子显示键位名，当前选中高亮）
            // 底部：摇杆实时位置 + hint
        });
    }
}
```

- 使用 `crossterm` 终端后端（跨平台且轻量）
- 每帧重新渲染，帧率限制在 30fps（避免浪费 CPU）
- 非活跃时不刷屏，保持终端清爽

---

## 九、Web 配置工具（技术方案）

### 9.1 用途

独立的配置可视化编辑工具，仅在 PC 浏览器上访问，不是运行必须组件。

### 9.2 HTTP Server

```rust
use axum::{
    routing::{get, post},
    Router,
};

async fn web_server(config: Arc<RwLock<Config>>) {
    let app = Router::new()
        .route("/", get(index_html))
        .route("/api/config", get(get_config))
        .route("/api/config", post(update_config))
        .route("/api/joystick", get(joystick_data))
        .route("/static/*path", get(static_files));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### 9.3 API 设计

| Method | Path | 说明 |
|--------|------|------|
| GET | `/` | 返回 index.html |
| GET | `/api/config` | 返回当前完整配置 JSON |
| POST | `/api/config` | 更新配置并触发热重载 |
| GET | `/api/joystick` | SSE 推送摇杆实时位置 |

### 9.4 静态文件

```
web/static/
├── index.html      # 布局 + 样式引用
├── style.css       # 全局样式
└── app.js          # Canvas 绘制 + 拖拽 + API 交互
```

- 零 npm 依赖
- Canvas 原生 2D API 绘制摇杆网格
- `fetch()` API 与后端通信

---

## 十、浮窗 UI（输入法候选窗风格）

### 10.1 需求

- 始终置顶显示
- 半透明背景
- 低开销，实时显示网格选中状态 + 按键提示
- **不是必须运行的组件**，核心引擎无此依赖

### 10.2 方案：GTK3（动态链接）

系统是 Ubuntu，GTK3 预装。浮窗作为独立 feature 编译，核心代码不依赖 GTK。

```toml
[dependencies]
gtk = { version = "0.18", features = ["v3_24"], optional = true }

[features]
default = []
overlay = ["gtk"]
```

**浮窗进程**：独立进程，通过 Unix socket 接收核心推送的实时状态数据。

```
JoyBoard Core (无 UI 依赖)
  ├── 主循环: 处理输入 → 引擎 → 输出
  └── 通过 Unix socket 推送状态给 GTK 进程（如启用）
        │
GTK 浮窗进程 (optional feature)
  ├── 无边框窗口 (WindowType::Popup)
  ├── 半透明 CSS 背景
  ├── DrawingArea 绘制网格 + 摇杆位置
  └── Fluxbox 配置: set_keep_above(true)
```

### 10.3 交叉编译

在设备上本地编译。

```bash
ssh joyboard
sudo apt install libgtk-3-dev
cargo build --release --features overlay
```

### 10.4 开发流程

- 日常开发在 x86 上编译 `core` + `tui`，覆盖 80% 逻辑
- 浮窗代码在设备上本地编译测试（改动频次低，增量编译快）

### 10.5 开发优先级

浮窗 UI **最后开发**。前期核心逻辑开发时通过 TUI 即可获得完整视觉反馈。

---

## 十一、依赖清单

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# core: 输入 + 输出
evdev = "0.12"         # Linux evdev 读取
uinput = "0.2"         # /dev/uinput 写入

# 可选 feature: TUI
ratatui = { version = "0.26", optional = true }
crossterm = { version = "0.27", optional = true }

# 可选 feature: Web 配置工具
axum = { version = "0.7", optional = true }
tower-http = { version = "0.5", features = ["fs"], optional = true }

# 可选 feature: GTK 浮窗
gtk = { version = "0.18", features = ["v3_24"], optional = true }

[features]
default = ["tui"]
full = ["tui", "web"]
tui = ["ratatui", "crossterm"]
web = ["axum", "tower-http"]
overlay = ["gtk"]
```

---

## 十二、编译与部署

```bash
# 编译单文件二进制
cargo build --release

# 编译最小化（无 Web、无 TUI）
cargo build --release --no-default-features

# 编译带 Web 配置 UI
cargo build --release --features web

# 编译带 GTK 浮窗
cargo build --release --features overlay

# 编译全部功能
cargo build --release --features "tui,web,overlay"

# 部署到掌机
scp target/release/joyboard user@device:/usr/local/bin/

# 添加 udev 规则 (/etc/udev/rules.d/99-joyboard.rules)
KERNEL=="uinput", MODE="0666", OPTIONS+="static_node=uinput"
KERNEL=="event*", SUBSYSTEM=="input", MODE="0666"
```

### 大小目标

| 功能集             | 目标大小 |
| ------------------ | -------- |
| core（无 TUI/Web） | ~3MB     |
| core + TUI         | ~5MB     |
| full               | ~8MB     |

> 基于 Rust 静态链接 + LTO + 去掉 debug symbol 后的估算值。

---

## 十三、配置热重载

配置文件路径：`~/.config/joyboard/config.toml`

```rust
pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    version: AtomicU64,   // 递增版本号，主循环检测
}

impl ConfigManager {
    fn reload(&self) -> io::Result<()> {
        let new_config = Config::load()?;
        let mut guard = self.config.write().unwrap();
        *guard = new_config;
        self.version.fetch_add(1, Ordering::Release);
        Ok(())
    }
}
```

- 主循环每帧检查 `version` 是否变化
- LUT 在检测到配置变化后重新预计算
- 模式、Layer、按键映射立即生效，无需重启

---

## 十四、Platform 目标

- **设备**：Anbernic 掌机（全志 H700 / Cortex-A53）
- **CPU**：Cortex-A53 (ARMv8-A, 4 核, aarch64)
- **OS**：Linux (kernel 4.9.170, PREEMPT)
- **显示**：终端 (tty) 或 Fluxbox
- **输入**：`/dev/input/event*` (evdev)
- **输出**：`/dev/uinput`
