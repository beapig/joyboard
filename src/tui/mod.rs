/// 独立终端 UI（读取 daemon 状态文件渲染）
///
/// 用法：`joyboard tui`
/// 依赖：后台 daemon 已运行（写入 `/tmp/joyboard-state.json`）

use crate::config::keymap::{BASE_LAYOUT, FN_LAYOUT};
use crate::engine::layer::Layer;
use crate::engine::WorkMode;
use crate::state;
use ratatui::{
    Frame,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io::{self};
use std::time::{Duration, Instant};

pub fn run() -> io::Result<()> {
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut last_dot = Instant::now();
    let mut frame_count = 0u64;
    let mut fps = 0u64;

    // 读取延迟状态（daemon 是否在运行）
    let mut daemon_alive = false;
    let mut last_seen = Instant::now();

    let render_result = loop {
        let now = Instant::now();
        let elapsed = last_dot.elapsed();

        // 读取状态文件
        let payload = state::read();

        // 检测 daemon 存活：3 秒内读到过状态即视为存活
        if payload.is_some() {
            daemon_alive = true;
            last_seen = now;
        } else if now.duration_since(last_seen) > Duration::from_secs(3) {
            daemon_alive = false;
        }

        // FPS 计算
        frame_count += 1;
        if elapsed >= Duration::from_secs(1) {
            fps = frame_count;
            frame_count = 0;
            last_dot = now;
        }

        let res = terminal.draw(|frame| {
            let area = frame.size();
            if area.width < 40 || area.height < 12 {
                return;
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(7),
                    Constraint::Length(3),
                ])
                .split(area);

            let top_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            if let Some(ref p) = payload {
                let mode = match p.mode.as_str() {
                    "Mouse" => WorkMode::Mouse,
                    "Gamepad" => WorkMode::Gamepad,
                    _ => WorkMode::Keyboard,
                };
                let layer = match p.layer.as_str() {
                    "Fn" => Layer::Fn,
                    _ => Layer::Base,
                };

                render_title(frame, chunks[0], &mode, &layer);
                render_grid(frame, top_chunks[0], true, &p.left_grid_selected, &layer);
                render_grid(frame, top_chunks[1], false, &p.right_grid_selected, &layer);
                render_joysticks(frame, chunks[2], p.left_joystick, p.right_joystick);
            } else {
                // 无数据：显示等待提示
                let wait_msg = if daemon_alive {
                    "等待 daemon 数据..."
                } else {
                    "daemon 未运行 — 请先启动 joyboard"
                };
                let text = vec![Line::from(Span::styled(
                    wait_msg,
                    Style::default().fg(Color::Yellow),
                ))];
                let block = Block::default()
                    .title(" JoyBoard ")
                    .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL);
                let p = Paragraph::new(text)
                    .block(block)
                    .alignment(Alignment::Center);
                frame.render_widget(p, area);
            }

            // FPS 显示
            let fps_text = format!(" {} FPS ", fps);
            let fps_span = Span::styled(fps_text, Style::default().fg(Color::DarkGray));
            frame.render_widget(
                Paragraph::new(Line::from(fps_span)).alignment(Alignment::Right),
                Rect::new(area.width.saturating_sub(12), 0, 12, 1),
            );
        });

        if res.is_err() {
            break res.map(|_| ());
        }

        // 60 FPS
        let frame_time = now.elapsed();
        if frame_time < Duration::from_micros(16_666) {
            std::thread::sleep(Duration::from_micros(16_666) - frame_time);
        }

        // 检测 Ctrl+C / q / ESC
        if crossterm::event::poll(Duration::from_millis(0)).unwrap_or(false) {
            if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap_or(
                crossterm::event::Event::Key(crossterm::event::KeyCode::Char(' ').into()),
            ) {
                match key.code {
                    crossterm::event::KeyCode::Char('q')
                    | crossterm::event::KeyCode::Esc
                    | crossterm::event::KeyCode::Char('c') => break Ok(()),
                    _ => {}
                }
            }
        }
    };

    let _ = crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen);
    let _ = crossterm::terminal::disable_raw_mode();
    render_result
}

fn render_title(frame: &mut Frame, area: Rect, mode: &WorkMode, layer: &Layer) {
    let mode_str = match mode {
        WorkMode::Keyboard => "KEYBOARD",
        WorkMode::Mouse => "MOUSE",
        WorkMode::Gamepad => "GAMEPAD",
    };
    let layer_str = match layer {
        Layer::Base => "Base",
        Layer::Fn => "FN",
    };

    let text = vec![Line::from(vec![
        Span::styled(" JoyBoard ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(mode_str, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("  Layer: "),
        Span::styled(layer_str, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ])];

    let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan));
    let p = Paragraph::new(text).block(block).alignment(Alignment::Left);
    frame.render_widget(p, area);
}

fn render_grid(
    frame: &mut Frame,
    area: Rect,
    is_left: bool,
    selected: &Option<usize>,
    layer: &Layer,
) {
    let layout = match layer {
        Layer::Base => &BASE_LAYOUT,
        Layer::Fn => &FN_LAYOUT,
    };

    let label = if is_left { " 左网格 " } else { " 右网格 " };
    let accent = if is_left { Color::Red } else { Color::Blue };
    let cell_w: usize = ((area.width as usize).saturating_sub(2)) / 5;
    let cell_w = cell_w.max(4).min(8);

    let mut lines: Vec<Line> = Vec::new();
    for row in 0..3 {
        let mut spans = Vec::new();
        for col in 0..5 {
            let cell_idx = row * 5 + col;
            let key_col = if is_left { col } else { col + 5 };
            let key_name = if row < 3 && key_col < 10 { layout[row][key_col] } else { "?" };

            let is_selected = selected == &Some(cell_idx);
            let display = if key_name.len() > cell_w.saturating_sub(2) {
                &key_name[..cell_w.saturating_sub(2)]
            } else {
                key_name
            };

            let cell = if is_selected {
                let content = format!("[{:^cw$}]", display, cw = cell_w.saturating_sub(2));
                Span::styled(content, Style::default().fg(Color::White).bg(accent).add_modifier(Modifier::BOLD))
            } else {
                let content = format!(" {:^cw$} ", display, cw = cell_w.saturating_sub(2));
                Span::styled(content, Style::default().fg(accent))
            };
            spans.push(cell);
        }
        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .title(label)
        .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(lines).block(block).alignment(Alignment::Center);
    frame.render_widget(p, area);
}

fn render_joysticks(frame: &mut Frame, area: Rect, left: [f32; 2], right: [f32; 2]) {
    let text = vec![Line::from(vec![
        Span::styled(
            format!(" LS: ({:+.3}, {:+.3}) ", left[0], left[1]),
            Style::default().fg(Color::Red),
        ),
        Span::styled(
            format!(" RS: ({:+.3}, {:+.3}) ", right[0], right[1]),
            Style::default().fg(Color::Blue),
        ),
    ])];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(text).block(block).alignment(Alignment::Center);
    frame.render_widget(p, area);
}
