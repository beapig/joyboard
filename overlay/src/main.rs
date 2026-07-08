/// X11 浮窗 UI — 单行 BAR 样式，高度 32px
/// 纯 Rust 实现，零系统依赖，支持交叉编译
///
/// 独立二进制：`joyboard-overlay`
/// 通过 `/tmp/joyboard-state.json` 读取 daemon 状态

use joyboard_core::config::keymap::{BASE_LAYOUT, FN_LAYOUT};
use joyboard_core::engine::layer::Layer;
use joyboard_core::engine::WorkMode;
use joyboard_core::state;
use std::time;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::xcb_ffi::XCBConnection;

const CELL_W: u16 = 48;
const CELL_H: u16 = 32;
const GRID_COLS: usize = 5;
const WIN_H: u16 = CELL_H;
const REFRESH_INTERVAL_MS: u64 = 33;
const CENTER_GAP: u16 = 100; // 中间间隔宽度，用于显示状态文本

// 颜色定义
const BG_COLOR: (u8, u8, u8) = (0x22, 0x22, 0x22);       // bar 背景
const CELL_BG: (u8, u8, u8) = (0x33, 0x33, 0x33);       // 格子填充
const CELL_BORDER: (u8, u8, u8) = (0x22, 0x22, 0x22);   // 格子边框
const TEXT_COLOR: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);    // 文字颜色
const SHIFT_COLOR: (u8, u8, u8) = (0x6D, 0xD4, 0x01);   // 上档字符颜色
const LEFT_HIGHLIGHT: (u8, u8, u8) = (0x9C, 0x29, 0x29); // 左高亮
const RIGHT_HIGHLIGHT: (u8, u8, u8) = (0x0C, 0x67, 0xAD); // 右高亮
const STATUS_NORMAL: (u8, u8, u8) = (0x8D, 0x8D, 0x8D);  // Joyboard 颜色
const STATUS_FN: (u8, u8, u8) = (0xF7, 0xB5, 0x01);      // Fn 颜色

struct OverlayState {
    mode: WorkMode,
    layer: Layer,
    left_selected: Option<usize>,
    right_selected: Option<usize>,
    shift: bool,
    capslock: bool,
}

fn display_name(name: &str, shift: bool, capslock: bool) -> &str {
    // 空格键始终显示 "sp"
    if name == " " {
        return "sp";
    }
    
    // 判断是否需要大写：shift 和 capslock 异或
    let upper = shift != capslock;
    
    if !upper {
        return name;
    }
    // 大写时，只有字母才显示大写，双字符格子保持原样
    match name {
        "a" => "A", "b" => "B", "c" => "C", "d" => "D", "e" => "E",
        "f" => "F", "g" => "G", "h" => "H", "i" => "I", "j" => "J",
        "k" => "K", "l" => "L", "m" => "M", "n" => "N", "o" => "O",
        "p" => "P", "q" => "Q", "r" => "R", "s" => "S", "t" => "T",
        "u" => "U", "v" => "V", "w" => "W", "x" => "X", "y" => "Y", "z" => "Z",
        s => s, // 其他保持原样，包括双字符格子如 "!1"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let top_align = std::env::args().any(|a| a == "-t");

    let (conn, screen_num) = XCBConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    let screen_w = screen.width_in_pixels;
    let screen_h = screen.height_in_pixels;
    let y_pos = if top_align { 0 } else { (screen_h as i16) - (WIN_H as i16) };

    let visual = screen.root_visual;
    let colormap = conn.generate_id()?;
    conn.create_colormap(
        ColormapAlloc::NONE,
        colormap,
        root,
        visual,
    )?
    .check()?;

    let win = conn.generate_id()?;

    let win_attrs = CreateWindowAux::new()
        .background_pixel(screen.black_pixel)
        .border_pixel(screen.black_pixel)
        .colormap(colormap);

    conn.create_window(
        screen.root_depth,
        win,
        root,
        0,
        y_pos as i16,
        screen_w,
        WIN_H,
        0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &win_attrs,
    )?
    .check()?;

    conn.change_window_attributes(
        win,
        &ChangeWindowAttributesAux::new().override_redirect(1),
    )?
    .check()?;

    let wm_type_atom = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
    let wm_type_dock = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DOCK")?.reply()?.atom;
    let wm_state_atom = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    let wm_state_above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;

    let dock_data: Vec<u8> = wm_type_dock.to_ne_bytes().to_vec();
    conn.change_property(
        PropMode::REPLACE,
        win,
        wm_type_atom,
        AtomEnum::ATOM,
        32,
        1,
        &dock_data,
    )?
    .check()?;

    let above_data: Vec<u8> = wm_state_above.to_ne_bytes().to_vec();
    conn.change_property(
        PropMode::REPLACE,
        win,
        wm_state_atom,
        AtomEnum::ATOM,
        32,
        1,
        &above_data,
    )?
    .check()?;

    // 保存 ABOVE 相关 atom，用于定期重新设置
    let above_atoms = (wm_state_atom, wm_state_above);

    conn.map_window(win)?
    .check()?;

    // 创建字体 GC
    let gc = conn.generate_id()?;
    conn.create_gc(
        gc,
        win,
        &CreateGCAux::new()
            .foreground(screen.white_pixel)
            .background(screen.black_pixel),
    )?
    .check()?;

    let font = conn.generate_id()?;
    // 尝试加载 18px bold 等宽字体
    let font_names = [
        "-misc-fixed-bold-r-normal--18-120-100-100-c-90-iso10646-1",
        "9x18bold",
        "-misc-fixed-medium-r-normal--18-120-100-100-c-90-iso10646-1",
        "fixed",
    ];
    let mut font_loaded = false;
    for name in font_names {
        if conn.open_font(font, name.as_bytes()).is_ok() {
            font_loaded = true;
            break;
        }
    }
    if !font_loaded {
        conn.open_font(font, b"fixed")?
            .check()?;
    }
    conn.change_gc(gc, &ChangeGCAux::new().font(font))?
        .check()?;

    let mut state = OverlayState {
        mode: WorkMode::Keyboard,
        layer: Layer::Base,
        left_selected: None,
        right_selected: None,
        shift: false,
        capslock: false,
    };

    let mut last_draw = time::Instant::now();
    let mut visible = true;
    let mut reassert_counter = 0u32;
    const REASSERT_INTERVAL: u32 = 80; // 每 80 次刷新（约 2.4 秒）重新设置 ABOVE

    loop {
        let now = time::Instant::now();
        if now.duration_since(last_draw) >= time::Duration::from_millis(REFRESH_INTERVAL_MS) {
            // 定期重新设置 ABOVE 状态，并强制 raise 窗口，防止 WM 重启后层级丢失
            reassert_counter += 1;
            if reassert_counter >= REASSERT_INTERVAL {
                reassert_counter = 0;
                let above_data: Vec<u8> = above_atoms.1.to_ne_bytes().to_vec();
                let _ = conn.change_property(
                    PropMode::REPLACE,
                    win,
                    above_atoms.0,
                    AtomEnum::ATOM,
                    32,
                    1,
                    &above_data,
                );
                // 强制将窗口提升到最上层
                let _ = conn.configure_window(win, &ConfigureWindowAux::new().stack_mode(StackMode::from(0u8)));
            }

            if let Some(data) = state::read() {
                let new_mode = match data.mode.as_str() {
                    "Mouse" => WorkMode::Mouse,
                    "Gamepad" => WorkMode::Gamepad,
                    _ => WorkMode::Keyboard,
                };
                state.mode = new_mode;
                state.layer = match data.layer.as_str() { "Fn" => Layer::Fn, _ => Layer::Base };
                state.left_selected = data.left_grid_selected;
                state.right_selected = data.right_grid_selected;
                state.shift = data.shift;
                state.capslock = data.capslock;

                let should_visible = matches!(new_mode, WorkMode::Keyboard);
                if should_visible != visible {
                    visible = should_visible;
                    if visible {
                        conn.map_window(win)?;
                    } else {
                        conn.unmap_window(win)?;
                    }
                }
            }

            if visible {
                draw_bar(&conn, win, gc, &state, screen_w as i16, screen.root_depth)?;
            }

            conn.flush()?;
            last_draw = now;
        }

        if conn.poll_for_event()?.is_none() {
            std::thread::sleep(time::Duration::from_millis(5));
        }
    }
}

fn draw_bar(
    conn: &XCBConnection,
    win: Window,
    gc: Gcontext,
    state: &OverlayState,
    win_w: i16,
    screen_depth: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let pixmap = conn.generate_id()?;
    conn.create_pixmap(
        screen_depth,
        pixmap,
        win,
        win_w as u16,
        WIN_H,
    )?
    .check()?;

    let grid_w = CELL_W as i16 * GRID_COLS as i16;
    let left_x = 0;
    let right_x = win_w - grid_w;
    let center_start = grid_w;
    let center_end = win_w - grid_w;

    // 背景
    fill_rect(conn, pixmap, gc, 0, 0, win_w, WIN_H as i16, BG_COLOR)?;

    // 左格子
    draw_grid(conn, pixmap, gc, state, true, left_x, LEFT_HIGHLIGHT)?;
    // 右格子
    draw_grid(conn, pixmap, gc, state, false, right_x, RIGHT_HIGHLIGHT)?;
    // 中间状态文本（在格子间区域居中）
    draw_status(conn, pixmap, gc, state, center_start, center_end)?;

    conn.copy_area(
        pixmap,
        win,
        gc,
        0,
        0,
        0,
        0,
        win_w as u16,
        WIN_H,
    )?
    .check()?;

    conn.free_pixmap(pixmap)?
        .check()?;

    Ok(())
}

fn draw_grid(
    conn: &XCBConnection,
    win: Window,
    gc: Gcontext,
    state: &OverlayState,
    is_left: bool,
    ox: i16,
    highlight: (u8, u8, u8),
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = match state.layer {
        Layer::Fn => &FN_LAYOUT,
        Layer::Base => &BASE_LAYOUT,
    };

    let cell_w = CELL_W as i16;
    let h = CELL_H as i16;
    let selected_idx = if is_left { state.left_selected } else { state.right_selected };
    let current_row = selected_idx.map(|idx| idx / GRID_COLS).unwrap_or(0);
    let current_col = selected_idx.map(|idx| idx % GRID_COLS);

    for col in 0..GRID_COLS {
        let cx = ox + col as i16 * cell_w;
        let key_col = if is_left { col } else { col + GRID_COLS };
        let key_name = layout[current_row][key_col];

        let is_selected = current_col == Some(col);

        // 格子背景
        let bg = if is_selected { highlight } else { CELL_BG };
        fill_rect(conn, win, gc, cx, 0, cell_w, h, bg)?;

        // 边框
        conn.change_gc(gc, &ChangeGCAux::new().foreground(rgb_to_pixel(CELL_BORDER.0, CELL_BORDER.1, CELL_BORDER.2)))?
            .check()?;
        conn.poly_line(
            CoordMode::ORIGIN,
            win,
            gc,
            &[
                Point { x: cx, y: 0 },
                Point { x: cx + cell_w, y: 0 },
                Point { x: cx + cell_w, y: h },
                Point { x: cx, y: h },
                Point { x: cx, y: 0 },
            ],
        )?
        .check()?;

        // 文字
        // 双字符格子始终两种颜色显示：主字符用 TEXT_COLOR，辅字符用 SHIFT_COLOR
        let display = display_name(key_name, state.shift, state.capslock);
        let key_bg = if is_selected { highlight } else { CELL_BG };

        // 设置文字颜色和背景透明
        conn.change_gc(gc, &ChangeGCAux::new()
            .foreground(rgb_to_pixel(TEXT_COLOR.0, TEXT_COLOR.1, TEXT_COLOR.2))
            .background(rgb_to_pixel(key_bg.0, key_bg.1, key_bg.2))
        )?
            .check()?;

        let cell_bg = key_bg;
        if is_double_char_key(key_name) && display.len() == 2 {
            // 双字符格子：第一个字符（上档）用 SHIFT_COLOR，第二个字符（下档）用 TEXT_COLOR
            // 例如 "!1" 显示为 "! 1"，"! " 是上档字符，"1" 是下档字符
            let chars: Vec<char> = display.chars().collect();
            let char1 = chars[0].to_string();
            let char2 = chars[1].to_string();

            let char_w = 10;
            let start_x = cx + (cell_w - char_w * 2) / 2;
            let text_y = h / 2 + 5;

            // 第一个字符（上档）- SHIFT_COLOR
            conn.change_gc(gc, &ChangeGCAux::new()
                .foreground(rgb_to_pixel(SHIFT_COLOR.0, SHIFT_COLOR.1, SHIFT_COLOR.2))
                .background(rgb_to_pixel(cell_bg.0, cell_bg.1, cell_bg.2))
            )?
                .check()?;
            conn.image_text8(win, gc, start_x, text_y, char1.as_bytes())?
                .check()?;

            // 第二个字符（下档）- TEXT_COLOR
            conn.change_gc(gc, &ChangeGCAux::new()
                .foreground(rgb_to_pixel(TEXT_COLOR.0, TEXT_COLOR.1, TEXT_COLOR.2))
                .background(rgb_to_pixel(cell_bg.0, cell_bg.1, cell_bg.2))
            )?
                .check()?;
            conn.image_text8(win, gc, start_x + char_w, text_y, char2.as_bytes())?
                .check()?;
        } else {
            // 普通格子
            let text_x = cx + cell_w / 2 - display.len() as i16 * 5;
            let text_y = h / 2 + 5;
            conn.image_text8(win, gc, text_x, text_y, display.as_bytes())?
                .check()?;
        }
    }

    Ok(())
}

// 判断是否为双字符格子（如 "!1", "@2" 等）
fn is_double_char_key(name: &str) -> bool {
    name.len() == 2 && name.chars().next().unwrap().is_ascii_punctuation()
}

fn draw_status(
    conn: &XCBConnection,
    win: Window,
    gc: Gcontext,
    state: &OverlayState,
    center_start: i16,
    center_end: i16,
) -> Result<(), Box<dyn std::error::Error>> {
    let fn_active = matches!(state.layer, Layer::Fn);
    let caps_active = state.capslock;

    let h = CELL_H as i16;
    let text_y = h / 2 + 5;

    // 清除中间区域背景
    let center_width = center_end - center_start;
    fill_rect(conn, win, gc, center_start, 0, center_width, h, BG_COLOR)?;

    // 重置 GC 的背景色为 BAR 背景色，避免继承格子的高亮背景色
    conn.change_gc(gc, &ChangeGCAux::new()
        .background(rgb_to_pixel(BG_COLOR.0, BG_COLOR.1, BG_COLOR.2))
    )?
        .check()?;

    // 计算每个词的宽度（fixed 18px 字体约 10px per char）
    const CHAR_W: i16 = 10;
    let joy_w = "Joyboard".len() as i16 * CHAR_W;
    let gap1_w = 4 * CHAR_W;  // Joyboard 和 Fn 之间的空格
    let fn_w = "Fn".len() as i16 * CHAR_W;
    let gap2_w = 3 * CHAR_W;  // Fn 和 Caps 之间的空格
    let caps_w = "Caps".len() as i16 * CHAR_W;

    let total_w = joy_w + gap1_w + fn_w + gap2_w + caps_w;
    let start_x = center_start + (center_width - total_w) / 2;

    let mut x = start_x;

    // Joyboard - 灰色
    conn.change_gc(gc, &ChangeGCAux::new()
        .foreground(rgb_to_pixel(STATUS_NORMAL.0, STATUS_NORMAL.1, STATUS_NORMAL.2))
    )?
        .check()?;
    conn.image_text8(win, gc, x, text_y, b"Joyboard")?
        .check()?;
    x += joy_w + gap1_w;

    // Fn - 根据状态选择颜色
    let fn_color = if fn_active { STATUS_FN } else { STATUS_NORMAL };
    conn.change_gc(gc, &ChangeGCAux::new()
        .foreground(rgb_to_pixel(fn_color.0, fn_color.1, fn_color.2))
    )?
        .check()?;
    conn.image_text8(win, gc, x, text_y, b"Fn")?
        .check()?;
    x += fn_w + gap2_w;

    // Caps - 根据状态选择颜色
    let caps_color = if caps_active { (0x6Du8, 0xD4u8, 0x01u8) } else { STATUS_NORMAL };
    conn.change_gc(gc, &ChangeGCAux::new()
        .foreground(rgb_to_pixel(caps_color.0, caps_color.1, caps_color.2))
    )?
        .check()?;
    conn.image_text8(win, gc, x, text_y, b"Caps")?
        .check()?;

    Ok(())
}

fn fill_rect(
    conn: &XCBConnection,
    win: Window,
    gc: Gcontext,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    color: (u8, u8, u8),
) -> Result<(), Box<dyn std::error::Error>> {
    conn.change_gc(gc, &ChangeGCAux::new().foreground(rgb_to_pixel(color.0, color.1, color.2)))?
        .check()?;
    conn.poly_fill_rectangle(
        win,
        gc,
        &[Rectangle { x, y, width: w as u16, height: h as u16 }],
    )?
    .check()?;
    Ok(())
}

fn rgb_to_pixel(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}