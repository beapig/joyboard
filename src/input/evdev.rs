use crate::input::{InputBackend, RawGamepadEvent};
use std::os::unix::io::AsRawFd;
use std::io;

pub struct EvdevBackend {
    device: evdev::Device,
}

impl EvdevBackend {
    pub fn new(path: &str) -> io::Result<Self> {
        let mut device = evdev::Device::open(path)
            .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;
        // 设为非阻塞模式，保证 60 FPS 循环不卡住
        let fd = device.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags >= 0 {
            unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK); }
        }
        Ok(Self { device })
    }
}

impl InputBackend for EvdevBackend {
    fn poll(&mut self) -> io::Result<Vec<RawGamepadEvent>> {
        let mut events = Vec::new();

        // 非阻塞模式下缓冲区为空返回 EAGAIN，视为无事件
        match self.device.fetch_events() {
            Ok(iter) => {
                for result in iter {
                    let event: evdev::InputEvent = result;
                    match event.kind() {
                        evdev::InputEventKind::Key(_) => {
                            events.push(RawGamepadEvent::Button {
                                code: event.code() as u32,
                                pressed: event.value() != 0,
                            });
                        }
                        evdev::InputEventKind::AbsAxis(_) => {
                            events.push(RawGamepadEvent::Axis {
                                axis: event.code() as u32,
                                value: event.value() as i16,
                            });
                        }
                        _ => {}
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // 缓冲区空，不是错误
            }
            Err(e) => return Err(e),
        }
        Ok(events)
    }
}
