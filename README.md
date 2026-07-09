# JoyBoard

面向 RG34XXSP Linux 掌机的手柄映射工具，在 X11 环境中将手柄按键映射为为虚拟的键盘和鼠标输入。

## 项目简介

JoyBoard 是一个纯 Rust 实现的手柄输入映射工具，专为 RG34XXSP 掌机设计。通过读取 `/dev/input/event*` 设备，将手柄的摇杆、按键事件转换为标准的键盘和鼠标输入，输出到 `/dev/uinput` 虚拟设备。

主要特点：

- 零系统依赖，支持交叉编译
- 三种工作模式：键盘模式、鼠标模式、手柄模式
- X11 浮窗 UI 实时显示当前按键状态
- Web 配置面板支持动态调整参数

## 功能介绍

### 工作模式

JoyBoard 支持三种工作模式，通过 Menu 键切换（长按=手柄模式和其他俩模式之间切换，短按=键盘和鼠标模式之间切换）：

| 模式                 | 说明                                                                                       |
| -------------------- | ------------------------------------------------------------------------------------------ |
| 手柄模式             | 所有按键透传，作为普通手柄使用                                                             |
| 键盘模式（默认模式） | ←→摇杆映射为一个虚拟矩阵键盘，其他按键映射为常用功能键与修饰键                             |
| 鼠标模式             | ←摇杆映射为鼠标光标移动，→摇杆映射为滚轮动作，其他按键映射为常用功能键与修饰键以及鼠标←→键 |

### 1. 键盘模式

#### 摇杆键盘映射

摇杆区域划分为←→两个 3×5 的网格，共 30 个虚拟按键：

**Base 层（默认）**：

```
+---+---+---+---+---+   +---+---+---+---+---+
| q | w | e | r | t |   | y | u | i | o | p |
+---+---+---+---+---+   +---+---+---+---+---+
| a | s | d | f | g |   | h | j | k | l | : |
+---+---+---+---+---+   +---+---+---+---+---+
| z | x | c | v | b |   | n | m | , | . | / |
+---+---+---+---+---+   +---+---+---+---+---+
```

**FN 层（L1 激活）**：

```
+----+----+----+----+-----+   +----+----+----+----+----+
| !1 | @2 | #3 | $4 | %5  |   | ^6 | &7 | *8 | (9 | )0 |
+----+----+----+----+-----+   +----+----+----+----+----+
| F1 | F2 | F3 | F4 | F5  |   | ~` | sp | -_ | "' | += |
+----+----+----+----+-----+   +----+----+----+----+----+
| F6 | F7 | F8 | F9 | F10 |   |F11 |F12 | [{ | ]} | \| |
+----+----+----+----+-----+   +----+----+----+----+----+
```

#### 物理按键映射

| 物理按键 | Base 层               | FN 层             |
| -------- | --------------------- | ----------------- |
| A        | Enter                 | Enter             |
| B        | Esc                   | Esc               |
| X        | Delete                | Delete            |
| Y        | Insert                | CapsLock          |
| L1       | FN 层切换（双击锁定） | 取消 FN 锁定      |
| L2       | Ctrl                  | Ctrl              |
| R1       | Backspace             | Delete            |
| R2       | Alt                   | Alt               |
| L3       | 锁定←摇杆当前格子     | 锁定←摇杆当前格子 |
| R3       | 锁定→摇杆当前格子     | 锁定→摇杆当前格子 |
| SE       | Tab / Shift（长按）   | Tab / Shift       |
| ST       | Space / Shift（长按） | Space / Shift     |

### 2. 鼠标模式

- **←摇杆**：鼠标指针移动
- **→摇杆**：滚轮（X轴水平滚动，Y轴垂直滚动）
- **L3**：进入精细移动模式（降低移动速度，松开有1秒延迟才退出）
- **L1**：精细移动模式（松开立即退出）
- **Y**：鼠标←键
- **X**：鼠标→键
- **R3**：鼠标中键

### 3. 手柄模式

所有按键直接透传到系统，作为标准手柄使用。长按 FN 键返回上一模式。

## 编译产物

### `joyboard` — 系统主服务

主程序，提供以下命令：

```bash
# 启动后台 daemon（读取配置中的设备路径）
joyboard

# 启动终端 UI（连接 daemon 显示状态）
joyboard tui

# 调试：打印逻辑摇杆/按键事件
joyboard evtest /dev/input/event1

# 调试：打印引擎输出的键盘事件
joyboard keytest /dev/input/event1

# 调试：多阶段管线 + 状态面板
joyboard debug /dev/input/event1

# 启动 Web 配置面板（默认端口 3000）
joyboard config [端口号] [evdev路径]
```

配置文件：`~/.config/joyboard/config.json`

### `joyboard-overlay` — UI 显示浮层

X11 单行 BAR 样式浮窗，实时显示当前按键状态和模式。

```bash
# 默认底部对齐
joyboard-overlay

# 顶部对齐
joyboard-overlay -t
```

**浮窗布局**：

- ←侧：←摇杆网格（5格）
- 中间：状态文本（Joyboard / Fn / Caps）
- →侧：→摇杆网格（5格）

浮窗通过 `/tmp/joyboard-state.json` 读取 daemon 的状态。

## 技术栈

- **Rust**：纯 Rust 实现，零 C 依赖
- **X11RB**：X11 协议绑定，用于 overlay 窗口
- **evdev**：Linux 输入设备读取
- **uinput**：虚拟输入设备输出
- **WebSocket**：Web 配置面板实时通信

## 编译

```bash
# 开发机交叉编译（ARM 架构）
cargo build --target=aarch64-unknown-linux-gnu --release

# 仅编译主程序（含 Web 配置）
cargo build --target=aarch64-unknown-linux-gnu --features web --release

# 仅编译 overlay
cargo build --target=aarch64-unknown-linux-gnu --release -p joyboard-overlay
```

## 部署

```bash
# 部署主程序
scp target/aarch64-unknown-linux-gnu/release/joyboard root@192.168.1.42:/usr/local/bin/

# 部署 overlay
scp target/aarch64-unknown-linux-gnu/release/joyboard-overlay root@192.168.1.42:/usr/local/bin/
```

## 配置

配置文件路径：`~/.config/joyboard/config.json`

主要配置项：

- `evdev_path`：手柄设备路径（如 `/dev/input/event1`）
- `joystick.deadzone`：摇杆死区设置
- `joystick.curve`：摇杆曲线类型（linear/quadratic/cubic）
- `mouse.sensitivity`：鼠标灵敏度
- `button_mode.hold_threshold_ms`：长按阈值（默认 400ms）

通过 Web 配置面板可以实时调整参数。
