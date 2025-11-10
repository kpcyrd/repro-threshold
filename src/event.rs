use crossterm::event::{EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;

pub enum Event {
    Yes,
    No,
    ScrollUp,
    ScrollDown,
    ScrollFirst,
    ScrollLast,
    Reload,
    Toggle,
    Quit,
}

impl Event {
    pub async fn read(stream: &mut EventStream) -> Option<Self> {
        let event = stream.next().await?.ok()?.as_key_press_event()?;
        match event.code {
            KeyCode::Char('y') => Some(Event::Yes),
            KeyCode::Char('n') => Some(Event::No),
            KeyCode::Char('k') | KeyCode::Up => Some(Event::ScrollUp),
            KeyCode::Char('j') | KeyCode::Down => Some(Event::ScrollDown),
            KeyCode::Char('g') | KeyCode::Home => Some(Event::ScrollFirst),
            KeyCode::Char('G') | KeyCode::End => Some(Event::ScrollLast),
            KeyCode::Char('r') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Event::Reload)
            }
            KeyCode::Char(' ') => Some(Event::Toggle),
            KeyCode::Char('q') => Some(Event::Quit),
            KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Event::Quit)
            }
            _ => None,
        }
    }
}
