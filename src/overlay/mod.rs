/// GTK 浮窗 UI（输入法候选窗风格）
///
/// 独立进程模式：`joyboard overlay`
/// 通过 `/tmp/joyboard-state.json` 与主进程通信

use crate::config::keymap::{BASE_LAYOUT, FN_LAYOUT};
use crate::engine::layer::Layer;
use crate::engine::WorkMode;
use crate::state;
use gtk::prelude::*;
use gtk::cairo;
use glib::Propagation;
use std::cell::RefCell;
use std::rc::Rc;

/// 启动 GTK 浮窗（`joyboard overlay` 命令入口）
pub fn run_overlay() {
    let application = gtk::Application::new(Some("com.joyboard.overlay"), Default::default());

    application.connect_activate(|app| {
        let window = gtk::ApplicationWindow::new(app);
        window.set_title("JoyBoard");
        window.set_decorated(false);
        window.set_keep_above(true);
        window.set_skip_taskbar_hint(true);
        window.set_resizable(false);
        window.set_default_size(420, 200);
        window.set_app_paintable(true);

        // CSS
        let css = b"
            window { background: rgba(10, 10, 20, 0.82); border: 1px solid #2a2a4a; border-radius: 4px; }
        ";
        let provider = gtk::CssProvider::new();
        let _ = provider.load_from_data(css);
        let screen = gtk::gdk::Screen::default().expect("no screen");
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let da = gtk::DrawingArea::new();
        da.set_size_request(420, 200);

        // 共享状态
        let overlay_state = Rc::new(RefCell::new(OverlayState {
            mode: WorkMode::Keyboard,
            layer: Layer::Base,
            left_selected: None,
            right_selected: None,
            left_joy: (0.0_f32, 0.0_f32),
            right_joy: (0.0_f32, 0.0_f32),
        }));

        // 33ms 轮询状态文件
        let poll_state = overlay_state.clone();
        let poll_da = da.clone();
        gtk::glib::timeout_add_local(
            std::time::Duration::from_millis(33),
            move || -> gtk::glib::ControlFlow {
                if let Some(data) = state::read() {
                    let mut s = poll_state.borrow_mut();
                    s.mode = match data.mode.as_str() {
                        "Mouse" => WorkMode::Mouse,
                        "Gamepad" => WorkMode::Gamepad,
                        _ => WorkMode::Keyboard,
                    };
                    s.layer = match data.layer.as_str() {
                        "Fn" => Layer::Fn,
                        _ => Layer::Base,
                    };
                    s.left_selected = data.left_grid_selected;
                    s.right_selected = data.right_grid_selected;
                    s.left_joy = (data.left_joystick[0], data.left_joystick[1]);
                    s.right_joy = (data.right_joystick[0], data.right_joystick[1]);
                    poll_da.queue_draw();
                }
                gtk::glib::ControlFlow::Continue
            },
        );

        // 绘制回调
        let draw_state = overlay_state.clone();
        da.connect_draw(move |_, cr| {
            let s = draw_state.borrow();
            draw_overlay(cr, 420.0, 200.0, &s);
            glib::Propagation::Stop
        });

        window.add(&da);
        window.show_all();
    });

    application.run();
}

struct OverlayState {
    mode: WorkMode,
    layer: Layer,
    left_selected: Option<usize>,
    right_selected: Option<usize>,
    left_joy: (f32, f32),
    right_joy: (f32, f32),
}

fn draw_overlay(cr: &cairo::Context, width: f64, height: f64, state: &OverlayState) {
    // 半透明背景
    cr.set_source_rgba(0.08, 0.08, 0.14, 0.82);
    cr.rectangle(0.0, 0.0, width, height);
    cr.fill();

    // 边框
    cr.set_source_rgba(0.16, 0.16, 0.29, 0.9);
    cr.set_line_width(1.0);
    cr.rectangle(0.5, 0.5, width - 1.0, height - 1.0);
    cr.stroke();

    // 标题
    let mode_str = match state.mode {
        WorkMode::Mouse => "MOUSE",
        WorkMode::Gamepad => "GAMEPAD",
        WorkMode::Keyboard => "KEYBOARD",
    };
    let layer_str = match state.layer {
        Layer::Fn => "FN",
        Layer::Base => "Base",
    };

    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(13.0);
    cr.set_source_rgba(0.0, 0.8, 0.8, 0.9);
    cr.move_to(12.0, 18.0);
    cr.show_text(&format!("JoyBoard  {}  Layer: {}", mode_str, layer_str));

    // 分隔线
    cr.set_source_rgba(0.2, 0.2, 0.4, 0.5);
    cr.move_to(8.0, 26.0);
    cr.line_to(width - 8.0, 26.0);
    cr.set_line_width(0.5);
    cr.stroke();

    // 左右网格
    cr.save();
    cr.translate(0.0, 30.0);
    let grid_h = height - 30.0 - 22.0;

    // 左网格标签
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(9.0);
    cr.set_source_rgba(0.9, 0.2, 0.3, 0.7);
    cr.move_to(12.0, 10.0);
    cr.show_text("L");

    // 右网格标签
    cr.set_source_rgba(0.2, 0.3, 0.9, 0.7);
    cr.move_to(width / 2.0 + 12.0, 10.0);
    cr.show_text("R");

    // 左网格
    draw_grid(
        cr,
        state,
        true,
        8.0,
        16.0,
        width / 2.0 - 20.0,
        grid_h - 20.0,
    );
    // 右网格
    draw_grid(
        cr,
        state,
        false,
        width / 2.0 + 8.0,
        16.0,
        width / 2.0 - 20.0,
        grid_h - 20.0,
    );
    cr.restore();

    // 底部：摇杆坐标
    let lx = state.left_joy.0;
    let ly = state.left_joy.1;
    let rx = state.right_joy.0;
    let ry = state.right_joy.1;

    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(9.0);
    cr.set_source_rgba(0.9, 0.2, 0.3, 0.7);
    cr.move_to(10.0, height - 6.0);
    cr.show_text(&format!("LS ({:+.2}, {:+.2})", lx, ly));
    cr.set_source_rgba(0.2, 0.3, 0.9, 0.7);
    cr.move_to(width / 2.0 + 10.0, height - 6.0);
    cr.show_text(&format!("RS ({:+.2}, {:+.2})", rx, ry));
}

fn draw_grid(
    cr: &cairo::Context,
    state: &OverlayState,
    is_left: bool,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let layout = match state.layer {
        Layer::Fn => &FN_LAYOUT,
        Layer::Base => &BASE_LAYOUT,
    };
    let accent_r = if is_left { 0.9 } else { 0.2 };
    let accent_g = if is_left { 0.2 } else { 0.3 };
    let accent_b = if is_left { 0.3 } else { 0.9 };

    let cell_w = w / 5.0;
    let cell_h = h / 3.0;

    for row in 0..3 {
        for col in 0..5 {
            let cx = x + col as f64 * cell_w;
            let cy = y + row as f64 * cell_h;
            let cell_idx = (row * 5 + col) as usize;
            let key_col = if is_left {
                col as usize
            } else {
                (col + 5) as usize
            };
            let key_name = if row < 3 && key_col < 10 {
                layout[row as usize][key_col]
            } else {
                "?"
            };

            let selected = if is_left {
                state.left_selected == Some(cell_idx)
            } else {
                state.right_selected == Some(cell_idx)
            };

            // 单元格背景
            if selected {
                cr.set_source_rgba(accent_r, accent_g, accent_b, 0.55);
            } else {
                cr.set_source_rgba(accent_r, accent_g, accent_b, 0.07);
            }
            cr.rectangle(cx, cy, cell_w, cell_h);
            cr.fill();

            // 单元格边框
            cr.set_source_rgba(accent_r, accent_g, accent_b, 0.2);
            cr.set_line_width(0.5);
            cr.rectangle(cx, cy, cell_w, cell_h);
            cr.stroke();

            // 按键名
            let text_size = 9.0;
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            cr.set_font_size(text_size);

            // 估算文本宽度
            let text_w = key_name.len() as f64 * text_size * 0.55;
            let text_x = cx + cell_w / 2.0 - text_w / 2.0;
            let text_y = cy + cell_h / 2.0 + text_size / 2.0 - 1.0;

            if selected {
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            } else {
                cr.set_source_rgba(accent_r, accent_g, accent_b, 0.55);
            }
            cr.move_to(text_x, text_y);
            cr.show_text(key_name);
        }
    }
}
