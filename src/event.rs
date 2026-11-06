use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent};

pub enum AppEvent {
    Key(KeyEvent),
    #[allow(dead_code)] // dimensions carried for future resize-aware layout
    Resize(u16, u16),
    Tick,
}

pub fn poll_event() -> std::io::Result<AppEvent> {
    // ponytail: 33 ms idle wake-up (~30 Hz) — tokens drained per tick appear
    // within one frame, imperceptible; halves idle CPU vs the old 16 ms poll.
    if event::poll(Duration::from_millis(33))? {
        match event::read()? {
            Event::Key(key) => Ok(AppEvent::Key(key)),
            Event::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
            _ => Ok(AppEvent::Tick),
        }
    } else {
        Ok(AppEvent::Tick)
    }
}
