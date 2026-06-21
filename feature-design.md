# JoyBoard 功能设计文档

> 本文档描述 JoyBoard 的所有功能行为，不涉及技术实现细节。
> 键位全部固定，用户仅能调整运行时参数（死区、灵敏度、时序阈值等）。

## 一、硬件输入处理

### 1.1 摇杆校准

程序启动时自动校准摇杆中心点：
- 采集启动后 1 秒内的摇杆静止值作为中心偏移
- 校准数据保存到本地文件，下次启动自动复用
- 用户无需手动校准

### 1.2 死区处理

摇杆存在物理漂移，需要设置死区避免误触发：
- **中心死区**：摇杆在中心附近小幅移动时不产生输出（默认 15%）
- **边缘死区**：摇杆推到极限时直接饱和为最大值（默认 95%）

### 1.3 响应曲线

摇杆移动距离与输出速度的关系可调节：
- **线性**：移动距离与输出成正比
- **二次曲线**：小幅移动更精细，大幅移动更快速（默认）
- **三次曲线**：精细区域更大，适合需要精确控制的场景

---

## 二、模式切换交互

JoyBoard 支持三种工作模式：

| 模式         | 说明                         |
| ------------ | ---------------------------- |
| **键盘模式** | 默认模式，摇杆映射为键盘矩阵 |
| **鼠标模式** | 摇杆映射为鼠标移动和滚轮     |
| **手柄模式** | 原始手柄数据直通，不做映射   |

模式切换方式（通过物理 Function 键）：

| 操作              | 效果                        |
| ----------------- | --------------------------- |
| **Function tap**  | 键盘 ↔ 鼠标模式切换         |
| **Function hold** | 手柄 ↔（键盘/鼠标）模式切换 |

**切换逻辑**：
- 当前键盘模式 → Function tap → 鼠标模式
- 当前鼠标模式 → Function tap → 键盘模式
- 当前键盘/鼠标模式 → Function hold → 手柄模式
- 当前手柄模式 → Function hold → 回到上一个非手柄模式（键盘或鼠标）
- 当前手柄模式 → Function tap → 透传给手柄（作为普通手柄按键）

---

## 三、手柄模式

原始手柄数据直接传递给系统，不做任何按键映射。

**适用场景**：
- 运行模拟器（需要原生手柄输入）
- 游戏场景
  
---

## 四、鼠标模式

### 4.2 摇杆移动特性

- **灵敏度**：摇杆推得越远，鼠标移动越快
- **加速度**：快速推动摇杆时，鼠标移动有额外加速
- **精细移动**：按住 L3 键时，鼠标移动速度降到 1/4，适合精确点击

### 4.3 按键映射

按键描述：

- 如果不写，默认为normal动作,定义的KEY的up和down事件与Button的up和down事件一致。
- tap 和 hold 会单独标出；以 tap = enter，hold = escape 为例。

| 按键        | 功能                                  | 功能（FN layer） |
| ----------- | ------------------------------------- | ---------------- |
| D-pad UP    | 方向上                                | PageUp           |
| D-pad Down  | 方向下                                | PageDown         |
| D-pad Left  | 方向左                                | Home             |
| D-pad Right | 方向右                                | End              |
| A           | Enter                                 |                  |
| B           | Esc                                   |                  |
| X           | 鼠标右键                              |                  |
| y           | 鼠标左键                              |                  |
| L1          | FN 键                                 |                  |
| L2          | Ctrl（修饰键）                        |                  |
| R1          | Backspace                             | Delete           |
| R2          | Alt（修饰键）                         |                  |
| SE          | Tab / Left Shift（见下方特殊判定）    |                  |
| ST          | Space / Right Shift（见下方特殊判定） |                  |
| L3          | 精细移动                              |                  |
| R3          | 鼠标中键                              |                  |
| 左摇杆      | 鼠标指针移动                          |                  |
| 右摇杆      | 鼠标滚轮滚动                          |                  |

---

## 五、键盘模式

### 键盘模式下的按键映射

| 按键        | 功能                                  | 功能（FN layer） |
| ----------- | ------------------------------------- | ---------------- |
| D-pad UP    | 方向上                                | PageUp           |
| D-pad Down  | 方向下                                | PageDown         |
| D-pad Left  | 方向左                                | Home             |
| D-pad Right | 方向右                                | End              |
| A           | Enter                                 |                  |
| B           | Esc                                   |                  |
| X           | Delete                                | PauseBreak       |
| y           | Ins                                   | Capslock         |
| L1          | FN 键                                 |                  |
| L2          | Ctrl（修饰键）                        |                  |
| R1          | Backspace                             | Delete           |
| R2          | Alt（修饰键）                         |                  |
| SE          | Tab / Left Shift（见下方特殊判定）    |                  |
| ST          | Space / Right Shift（见下方特殊判定） |                  |
| L3          | L摇杆矩阵按键按下释放                 |                  |
| R3          | R摇杆矩阵按键按下释放                 |                  |
| 左摇杆      | L摇杆矩阵选择                         |                  |
| 右摇杆      | R摇杆矩阵选择                         |                  |


### 5.3 摇杆网格顶点系统

每个摇杆使用 **6×4 顶点系统**定义 5×3 个映射格子。顶点坐标与摇杆逻辑坐标系一致，但顶点范围可大于摇杆行程，实现"无穷大映射区"——摇杆推到极值时仍能稳定命中边缘格子。

```
a───b───c───d───e───f
│ A │ B │ C │ D │ E │
g───h───i───j───k───l
│ F │ G │ H │ I │ J │
m───n───o───p───q───r
│ K │ L │ M │ N │ O │
s───t───u───v───w───x
```

- **小写字母**（a~x）：24 个顶点，可独立配置坐标
- **大写字母**（A~O）：15 个映射格子
- **H**：摇杆初始位置（中心点）
- 顶点坐标超出摇杆行程范围时，对应行列的格子会延伸到无穷远，摇杆推到该方向时稳定选中边缘格子

**格子选中判定**：摇杆当前位置 `(x, y)` 落在对应四边形内即视为选中该格子。四边形内点判定使用叉积法。

### 5.4 Base 层键位表

|  L1   |  L2   |  L3   |  L4   |  L5   |  R1   |  R2   |  R3   |  R4   |  R5   |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
|   q   |   w   |   e   |   r   |   t   |   y   |   u   |   i   |   o   |   p   |
|   a   |   s   |   d   |   f   |   g   |   h   |   j   |   k   |   l   |  ;:   |
|   z   |   x   |   c   |   v   |   b   |   n   |   m   |  ,<   |  .>   |  /?   |


### 5.5 FN 层键位表

|  L1   |  L2   |  L3   |  L4   |  L5   |  R1   |    R2    |  R3   |  R4   |  R5   |
| :---: | :---: | :---: | :---: | :---: | :---: | :------: | :---: | :---: | :---: |
|  !1   |  @2   |  #3   |  $4   |  %5   |  ^6   |    &7    |  *8   |  (9   |  )0   |
|  F1   |  F2   |  F3   |  F4   |  F5   |  ~`   | <空格键> |  _-   |  "'   |  +=   |
|  F6   |  F7   |  F8   |  F9   |  F10  |  F11  |   F12    |  {[   |  }]   |  \|   |

> Row 2 的 R2 位置为空格键（用于实现空格键长按的场景）。


### 5.6 网格触发机制

- **L3**：左网格触发键
- **R3**：右网格触发键

**交互流程**：
1. 摇杆移动 → 选中对应格子（空间导航）
2. 按下 L3/R3 → 发送对应格子按键按下的的指令，**同时锁定当前格子的选中状态，这时即使再移动摇杆也不会更新焦点**，直到松开 LR3；
3，松开 L3/R3 → 发送对应格子按键松开的指令

---

## 六、按键时序交互

### 6.1 Normal 模式

按下即触发，松开即结束。用于修饰键（L2=Ctrl、R2=Alt）。

### 6.2 Extended 模式（tap-hold）

支持短按和长按两种触发：

| 操作            | 触发条件          | 效果                     |
| --------------- | ----------------- | ------------------------ |
| **tap**         | 按住时间 < 180ms  | 释放时触发 tap 动作      |
| **hold**        | 按住时间 > 400ms  | 到达阈值时触发 hold 动作 |
| **extend-hold** | 按住时间 > 1200ms | 触发 extend-hold 动作    |

**注意**：如果已触发 hold，释放时不再触发 tap。

### 6.3 Double-tap 模式

在 300ms 内收到两次短按则触发 double-tap 动作。

**应用**：L1 双击触发 FN Lock。

### 6.4 SE/ST 状态机

SE 和 ST 各有一个独立的状态机，互不干扰。每个状态机控制一种键值（Tab/Space）和对应的 Shift（Left/Right）。

#### 状态定义

| 状态       | 说明                                                               |
| ---------- | ------------------------------------------------------------------ |
| IDLE       | 未按下                                                             |
| WAITING    | 已按下，hold 定时器运行中，未做决定                                |
| KEY_DOWN   | 已决定为**键值角色**（Tab/Space 已按下保持），永不触发自己的 Shift |
| SHIFT_DOWN | 已决定为**Shift 角色**（Shift 已按下保持），永不触发自己的键值     |

#### 状态转移规则

**SE_down 时（检查 ST 状态）：**

| ST 状态    | 行为                                                                                              |
| ---------- | ------------------------------------------------------------------------------------------------- |
| IDLE       | SE → WAITING                                                                                      |
| WAITING    | 发 **Right_Shift_down**（ST 的 Shift）+ **Tab_down**（SE 的键）<br>ST → SHIFT_DOWN, SE → KEY_DOWN |
| SHIFT_DOWN | 发 **Tab_down**, SE → KEY_DOWN（ST 已提供 Shift，SE 成为纯 Tab）                                  |
| KEY_DOWN   | SE → WAITING（ST 已被用做键值，无法提供 Shift，SE 走正常判定）                                    |

**ST_down 时（检查 SE 状态）：**

| SE 状态    | 行为                                                                                               |
| ---------- | -------------------------------------------------------------------------------------------------- |
| IDLE       | ST → WAITING                                                                                       |
| WAITING    | 发 **Left_Shift_down**（SE 的 Shift）+ **Space_down**（ST 的键）<br>SE → SHIFT_DOWN, ST → KEY_DOWN |
| SHIFT_DOWN | 发 **Space_down**, ST → KEY_DOWN                                                                   |
| KEY_DOWN   | ST → WAITING                                                                                       |

**第三方按键按下时（检查 SE/ST 状态）：**

| SE/ST 状态 | 行为                                                         |
| ---------- | ------------------------------------------------------------ |
| WAITING    | 提前发送对应 Shift_down，进入 SHIFT_DOWN（取消 hold 定时器） |
| SHIFT_DOWN | 不重复发送 Shift，正常触发第三方按键                         |
| KEY_DOWN   | 不处理 Shift，正常触发第三方按键                             |

**SE/ST button_up 时：**

| 当前状态   | 行为                          |
| ---------- | ----------------------------- |
| WAITING    | tap：key_down + key_up → IDLE |
| SHIFT_DOWN | Shift_up → IDLE               |
| KEY_DOWN   | key_up → IDLE                 |

#### 场景示例

**场景 1：SE tap（超时前释放）**

```
SE: WAITING
T=50ms: SE_up → Tab_down + Tab_up → IDLE
```

**场景 2：SE hold（超时后释放）**

```
SE: WAITING
T=500ms: 超时 → Left_Shift_down → SHIFT_DOWN
T=1s:    SE_up → Left_Shift_up → IDLE
```

**场景 3：SE + A（第三方按键打断 WAITING）**

```
SE: WAITING
T=50ms: A_down → 检测 SE WAITING → Left_Shift_down（提前）
         SE → SHIFT_DOWN
         → A_down
T=100ms: A_up → A_up
T=150ms: SE_up → Left_Shift_up → IDLE
```

**场景 4：SE 先按 → ST 再按（双方本在 WAITING）**

```
T=0:    SE_down → WAITING
T=50ms: ST_down → 检测 SE WAITING
         → SE: SHIFT_DOWN（ST 触发了 Left_Shift）
         → ST: 发 Left_Shift_down + Space_down → KEY_DOWN
T=100ms: ST_up → Space_up（ST: KEY_DOWN → IDLE）
T=150ms: SE_up → Left_Shift_up（SE: SHIFT_DOWN → IDLE）
```

**场景 5：ST 先按 → SE 再按（双方本在 WAITING）**

```
T=0:    ST_down → WAITING
T=50ms: SE_down → 检测 ST WAITING
         → ST: SHIFT_DOWN（SE 触发了 Right_Shift）
         → SE: 发 Right_Shift_down + Tab_down → KEY_DOWN
T=100ms: SE_up → Tab_up（SE: KEY_DOWN → IDLE）
T=150ms: ST_up → Right_Shift_up（ST: SHIFT_DOWN → IDLE）
```

**场景 6：SE 已 SHIFT_DOWN → ST 再按**

```
T=0:    SE_down → WAITING
T=50ms: A_down → Left_Shift_down → SE: SHIFT_DOWN
T=80ms: ST_down → 检测 SE SHIFT_DOWN
         → ST: 发 Space_down → KEY_DOWN（SE 已提供 Shift，ST 成为纯 Space）
T=120ms: 第三方 A_up
T=200ms: ST_up → Space_up（ST: KEY_DOWN → IDLE）
T=300ms: SE_up → Left_Shift_up（SE: SHIFT_DOWN → IDLE）
```

**场景 7：SE 已 KEY_DOWN → ST 再按**

```
T=0:    ST_down → WAITING
T=50ms: SE_down → 检测 ST WAITING
         → ST: SHIFT_DOWN（SE 触发了 Right_Shift）
         → SE: 发 Right_Shift_down + Tab_down → KEY_DOWN
T=80ms: ST2_down → 检测 SE KEY_DOWN → ST2: WAITING（回退，正常走判定）
```

**场景 8：SE_WAITING + ST_WAITING → 第三方 A_down**

```
T=0:    SE_down → WAITING
T=20ms: ST_down → WAITING
T=50ms: A_down → 检测 SE WAITING → SE: SHIFT_DOWN + Left_Shift_down
                 检测 ST WAITING → ST: SHIFT_DOWN + Right_Shift_down
         → A_down
T=100ms: A_up → A_up
T=150ms: SE_up → Left_Shift_up → IDLE
T=200ms: ST_up → Right_Shift_up → IDLE

结果：Left_Shift + Right_Shift + A
```

**场景 9：一方 WAITING 超时 + 另一方后来按下**

```
T=0:    SE_down → WAITING
T=500ms: SE 超时 → Left_Shift_down → SHIFT_DOWN
T=600ms: ST_down → 检测 SE SHIFT_DOWN
         → ST: 发 Space_down → KEY_DOWN
T=700ms: ST_up → Space_up
T=800ms: SE_up → Left_Shift_up → IDLE

SE 先 hold 触发 Shift，ST 后来按下成为纯 Space。
```

---

## 七、UI 界面

### 7.1 终端 TUI（运行时）

终端内显示当前状态（ratatui 实现）：

```
┌─ JoyBoard ─────────────────────────┐
│ Mode: KEYBOARD  Layer: Base        │
├────────────────────────────────────┤
│ 左网格:        右网格:              │
│ [q] [w] [e]    [y] [u] [i]         │
│ [a] [s] [=]    [h] [j] [k]         │  ← [=] 表示当前选中
│ [z] [x] [c]    [n] [m] [,]         │
├────────────────────────────────────┤
│ 左摇杆: ( 0.23, -0.45 )            │
│ 右摇杆: ( 0.87,  0.12 )            │
└────────────────────────────────────┘
```

显示内容：
- 当前工作模式
- 当前 Layer
- 左右网格选中状态
- 摇杆实时位置

### 7.2 Web 配置工具（可选）

独立于核心程序的配置工具，用于可视化编辑配置文件、调试摇杆网格映射。不是运行必须组件。

**功能**：
- 可视化编辑 `config.toml` 所有配置项
- 拖拽或数值编辑摇杆网格顶点（实时预览网格形状变化）
- 摇杆行程测试：实时显示摇杆 XY 值，在网格图上标记当前位置
- 配置保存、热重载触发

**技术方案**：
- Rust 内嵌 HTTP Server（axum），绑定 `localhost`
- 纯静态 HTML + CSS + 原生 JS，零前端依赖
- Canvas 2D 绘制网格图（顶点拖拽、连线、摇杆位置标记）
- REST API 读/写配置文件、接收摇杆实时数据

---

## 八、配置文件

配置文件路径：`~/.config/joyboard/config.toml`

**可配置项**（仅运行时参数，不包含键位）：

| 配置项                               | 说明                   | 默认值              |
| ------------------------------------ | ---------------------- | ------------------- |
| `evdev_path`                         | 手柄设备路径           | `/dev/input/event0` |
| `log_level`                          | 日志级别               | `info`              |
| `joystick.deadzone.center`           | 中心死区               | `0.15`              |
| `joystick.deadzone.edge`             | 边缘死区               | `0.95`              |
| `joystick.curve.type`                | 响应曲线类型           | `quadratic`         |
| `joystick.curve.power`               | 曲线指数               | `2.0`               |
| `mouse.sensitivity`                  | 鼠标灵敏度             | `1.0`               |
| `mouse.acceleration`                 | 是否启用加速度         | `true`              |
| `mouse.fine_control.dpi_scale`       | 精细移动 DPI 缩放      | `0.25`              |
| `button_mode.tap_threshold_ms`       | tap 判定时间           | `180`               |
| `button_mode.hold_threshold_ms`      | hold 判定时间          | `400`               |
| `button_mode.extend_threshold_ms`    | extend-hold 判定时间   | `1200`              |
| `button_mode.double_tap_interval_ms` | 双击间隔               | `300`               |

**摇杆网格顶点配置**：左右摇杆各 24 个顶点（6×4），按行优先排列。顶点坐标可超出 `[-1, 1]` 范围，实现无穷大映射区。

```toml
[joystick_grid.left]
vertices = [
  # 行 0 (y = -1.0)
  [-1.0, -1.0 ], [-0.6, -1.0 ], [-0.2, -1.0 ],  [ 0.2, -1.0 ], [ 0.6, -1.0 ], [ 1.0, -1.0 ],
  # 行 1 (y = -0.33)
  [-1.0, -0.33], [-0.6, -0.33], [-0.2, -0.33],  [ 0.2, -0.33], [ 0.6, -0.33], [ 1.0, -0.33],
  # 行 2 (y = 0.33)
  [-1.0,  0.33], [-0.6,  0.33], [-0.2,  0.33],  [ 0.2,  0.33], [ 0.6,  0.33], [ 1.0,  0.33],
  # 行 3 (y = 1.0)
  [-1.0,  1.0 ], [-0.6,  1.0 ], [-0.2,  1.0 ],  [ 0.2,  1.0 ], [ 0.6,  1.0 ], [ 1.0,  1.0 ],
]

[joystick_grid.right]
vertices = [
  # 与左摇杆对称
  [-1.0, -1.0 ], [-0.6, -1.0 ], [-0.2, -1.0 ],  [ 0.2, -1.0 ], [ 0.6, -1.0 ], [ 1.0, -1.0 ],
  [-1.0, -0.33], [-0.6, -0.33], [-0.2, -0.33],  [ 0.2, -0.33], [ 0.6, -0.33], [ 1.0, -0.33],
  [-1.0,  0.33], [-0.6,  0.33], [-0.2,  0.33],  [ 0.2,  0.33], [ 0.6,  0.33], [ 1.0,  0.33],
  [-1.0,  1.0 ], [-0.6,  1.0 ], [-0.2,  1.0 ],  [ 0.2,  1.0 ], [ 0.6,  1.0 ], [ 1.0,  1.0 ],
]
```

配置修改后，发送 `SIGHUP` 信号或重启程序生效。