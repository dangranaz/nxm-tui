use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent};

pub enum AppEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
}

pub fn poll_event() -> std::io::Result<AppEvent> {
    if event::poll(Duration::from_millis(16))? {
        match event::read()? {
            Event::Key(key) => Ok(AppEvent::Key(key)),
            Event::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
            _ => Ok(AppEvent::Tick),
        }
    } else {
        Ok(AppEvent::Tick)
    }
}
