use crate::output::{Action, OutputBackend};
use std::io;

#[cfg(target_os = "linux")]
pub struct UInputBackend {
    device: uinput::Device,
}

#[cfg(target_os = "linux")]
use uinput::event::keyboard::Keyboard;
#[cfg(target_os = "linux")]
use uinput::event::relative::Position as RelPos;
#[cfg(target_os = "linux")]
use uinput::event::relative::Wheel as RelWheel;
#[cfg(target_os = "linux")]
use uinput::event::Event;
#[cfg(target_os = "linux")]
use uinput::event::{Controller, Keyboard as KbdEv, Relative};

#[cfg(target_os = "linux")]
impl UInputBackend {
    pub fn new() -> io::Result<Self> {
        use uinput::event::controller::Mouse as MButton;
        use uinput::event::keyboard::Key;

        let mut builder = uinput::default()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .name("JoyBoard Virtual Input")
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .vendor(0x1234)
            .product(0x5678)
            .version(1);

        // 注册所有键盘按键
        let key_variants = [
            Key::Reserved, Key::Esc, Key::_1, Key::_2, Key::_3,
            Key::_4, Key::_5, Key::_6, Key::_7, Key::_8, Key::_9, Key::_0,
            Key::Minus, Key::Equal, Key::BackSpace, Key::Tab,
            Key::Q, Key::W, Key::E, Key::R, Key::T, Key::Y,
            Key::U, Key::I, Key::O, Key::P,
            Key::LeftBrace, Key::RightBrace, Key::Enter,
            Key::LeftControl, Key::A, Key::S, Key::D, Key::F, Key::G,
            Key::H, Key::J, Key::K, Key::L,
            Key::SemiColon, Key::Apostrophe, Key::Grave,
            Key::LeftShift, Key::BackSlash,
            Key::Z, Key::X, Key::C, Key::V, Key::B,
            Key::N, Key::M, Key::Comma, Key::Dot, Key::Slash,
            Key::RightShift, Key::LeftAlt, Key::Space, Key::CapsLock,
            Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6,
            Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
            Key::RightControl, Key::RightAlt, Key::Home, Key::Up,
            Key::PageUp, Key::Left, Key::Right, Key::End, Key::Down,
            Key::PageDown, Key::Insert, Key::Delete,
            Key::LeftMeta, Key::RightMeta,
            Key::F13, Key::F14, Key::F15, Key::F16,
            Key::F17, Key::F18, Key::F19, Key::F20,
            Key::F21, Key::F22, Key::F23, Key::F24,
        ];
        for key in &key_variants {
            builder = builder
                .event(Event::Keyboard(KbdEv::Key(*key)))
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }

        // 注册鼠标按键
        for btn in &[MButton::Left, MButton::Right, MButton::Middle, MButton::Side] {
            builder = builder
                .event(Event::Controller(Controller::Mouse(*btn)))
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }

        // 注册鼠标相对移动
        builder = builder
            .event(Event::Relative(Relative::Position(RelPos::X)))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .event(Event::Relative(Relative::Position(RelPos::Y)))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .event(Event::Relative(Relative::Wheel(RelWheel::Vertical)))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let device = builder
            .create()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(Self { device })
    }
}

#[cfg(target_os = "linux")]
impl OutputBackend for UInputBackend {
    fn emit(&mut self, actions: &[Action]) -> io::Result<()> {
        for action in actions {
            match action {
                Action::KeyDown(code) => self.key(*code, 1)?,
                Action::KeyUp(code) => self.key(*code, 0)?,
                Action::MouseMove { dx, dy } => {
                    self.device
                        .send(Event::Relative(Relative::Position(RelPos::X)), *dx as i32)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    self.device
                        .send(Event::Relative(Relative::Position(RelPos::Y)), *dy as i32)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                }
                Action::MouseWheel(delta) => {
                    self.device
                        .send(Event::Relative(Relative::Wheel(RelWheel::Vertical)), *delta)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                }
            }
        }
        if !actions.is_empty() {
            self.device
                .synchronize()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
use uinput::event::keyboard::Key;

#[cfg(target_os = "linux")]
use uinput::event::controller::Mouse as MouseBtn;

#[cfg(target_os = "linux")]
impl UInputBackend {
    fn key(&mut self, code: u16, value: i32) -> io::Result<()> {
        // 鼠标按键 (BTN_LEFT/RIGHT/MIDDLE)
        if code >= 272 && code <= 274 {
            return self.mouse_btn(code, value);
        }
        if let Some(key) = key_from_code(code) {
            self.device
                .send(Event::Keyboard(KbdEv::Key(key)), value)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
        Ok(())
    }

    fn mouse_btn(&mut self, code: u16, value: i32) -> io::Result<()> {
        let btn = match code {
            272 => MouseBtn::Left,
            273 => MouseBtn::Right,
            274 => MouseBtn::Middle,
            _ => return Ok(()),
        };
        self.device
            .send(Event::Controller(Controller::Mouse(btn)), value)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }
}

/// u16 key code → uinput Key 枚举映射
#[cfg(target_os = "linux")]
fn key_from_code(code: u16) -> Option<Key> {
    use self::Key::*;
    Some(match code {
        1 => Esc, 2 => _1, 3 => _2, 4 => _3, 5 => _4,
        6 => _5, 7 => _6, 8 => _7, 9 => _8, 10 => _9, 11 => _0,
        12 => Minus, 13 => Equal, 14 => BackSpace, 15 => Tab,
        16 => Q, 17 => W, 18 => E, 19 => R, 20 => T,
        21 => Y, 22 => U, 23 => I, 24 => O, 25 => P,
        26 => LeftBrace, 27 => RightBrace, 28 => Enter,
        29 => LeftControl, 30 => A, 31 => S, 32 => D, 33 => F, 34 => G,
        35 => H, 36 => J, 37 => K, 38 => L,
        39 => SemiColon, 40 => Apostrophe, 41 => Grave,
        42 => LeftShift, 43 => BackSlash,
        44 => Z, 45 => X, 46 => C, 47 => V, 48 => B,
        49 => N, 50 => M, 51 => Comma, 52 => Dot, 53 => Slash,
        54 => RightShift, 56 => LeftAlt,
        57 => Space, 58 => CapsLock,
        59 => F1, 60 => F2, 61 => F3, 62 => F4,
        63 => F5, 64 => F6, 65 => F7, 66 => F8,
        67 => F9, 68 => F10, 87 => F11, 88 => F12,
        97 => RightControl, 100 => RightAlt,
        102 => Home, 103 => Up, 104 => PageUp,
        105 => Left, 106 => Right, 107 => End, 108 => Down, 109 => PageDown,
        110 => Insert, 111 => Delete,
        119 => Delete, // PauseBreak not in uinput::Key enum, use Delete as fallback
        _ => return None,
    })
}

#[cfg(not(target_os = "linux"))]
pub struct UInputBackend;

#[cfg(not(target_os = "linux"))]
impl UInputBackend {
    pub fn new() -> io::Result<Self> {
        Ok(Self)
    }
}

#[cfg(not(target_os = "linux"))]
impl OutputBackend for UInputBackend {
    fn emit(&mut self, _actions: &[Action]) -> io::Result<()> {
        Ok(())
    }
}
