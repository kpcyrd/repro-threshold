use crate::config::Config;
use crate::errors::*;
use crate::event::Event;
use crate::rebuilder::{self, Rebuilder, Selectable};
use crossterm::event::EventStream;
use ratatui::{DefaultTerminal, widgets::ListState};

#[derive(Debug)]
pub enum View {
    Home,
    Rebuilders { scroll: ListState },
}

impl View {
    pub const fn home() -> Self {
        View::Home
    }

    pub fn rebuilders() -> Self {
        let mut scroll = ListState::default();
        scroll.select_first();
        View::Rebuilders { scroll }
    }
}

#[derive(Debug)]
pub struct App {
    pub view: Option<View>,
    // Keep this state even when switching views
    pub home_scroll: ListState,
    pub confirm: bool,
    pub config: Config,
    pub rebuilders: Vec<Selectable<Rebuilder>>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let mut home_scroll = ListState::default();
        home_scroll.select_first();
        let mut app = Self {
            view: Some(View::home()),
            home_scroll,
            confirm: false,
            config,
            rebuilders: vec![],
        };
        app.rebuilders = app.config.resolve_rebuilder_view();
        app
    }

    pub fn scroll(&mut self) -> &mut ListState {
        match &mut self.view {
            Some(View::Rebuilders { scroll }) => scroll,
            _ => &mut self.home_scroll,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut events = EventStream::new();

        while self.view.is_some() {
            terminal.draw(|frame| {
                frame.render_widget(&mut self, frame.area());
            })?;

            match Event::read(&mut events).await {
                Some(Event::Yes) => {
                    if self.confirm {
                        // handle yes action
                        self.confirm = false;
                    }
                }
                Some(Event::No) => {
                    /*
                    if self.confirm {
                        // handle no action
                        self.confirm = false;
                    }
                    */
                    // TODO: dummy code, open the prompt
                    self.confirm = true;
                }
                Some(Event::ScrollUp) => {
                    self.scroll().select_previous();
                }
                Some(Event::ScrollDown) => {
                    self.scroll().select_next();
                }
                Some(Event::ScrollFirst) => {
                    self.scroll().select_first();
                }
                Some(Event::ScrollLast) => {
                    self.scroll().select_last();
                }
                Some(Event::Reload) => {
                    if let Some(View::Rebuilders { .. }) = self.view {
                        let list = rebuilder::fetch_rebuilderd_community().await?;
                        self.config.cached_rebuilderd_community = list;
                        self.config.save().await?;

                        self.rebuilders = self.config.resolve_rebuilder_view();
                    }
                }
                Some(Event::Toggle) => {
                    if let Some(View::Rebuilders { scroll }) = self.view
                        && let Some(idx) = scroll.selected()
                        && let Some(rebuilder) = self.rebuilders.get_mut(idx)
                    {
                        if rebuilder.active {
                            self.config
                                .trusted_rebuilders
                                .retain(|r| r.url != rebuilder.item.url);
                        } else {
                            self.config.trusted_rebuilders.push(rebuilder.item.clone());
                        }
                        self.config.save().await?;

                        rebuilder.active = !rebuilder.active;
                    }
                }
                Some(Event::Enter) => {
                    if let Some(View::Home) = self.view {
                        match self.home_scroll.selected() {
                            Some(0) => (),
                            Some(1) => {
                                self.view = Some(View::rebuilders());
                                self.rebuilders = self.config.resolve_rebuilder_view();
                            }
                            Some(2) => (), // TODO
                            Some(3) => self.view = None,
                            _ => {}
                        }
                    }
                }
                Some(Event::Plus) => {
                    if let Some(View::Home) = self.view
                        && self.home_scroll.selected() == Some(0)
                    {
                        let threshold = &mut self.config.rules.required_threshold;
                        *threshold = threshold.saturating_add(1);
                        self.config.save().await?;
                    }
                }
                Some(Event::Minus) => {
                    if let Some(View::Home) = self.view
                        && self.home_scroll.selected() == Some(0)
                    {
                        let threshold = &mut self.config.rules.required_threshold;
                        *threshold = threshold.saturating_sub(1);
                        self.config.save().await?;
                    }
                }
                Some(Event::Esc) => {
                    self.view = Some(View::home());
                }
                Some(Event::Quit) => {
                    self.view = if let Some(View::Home) = self.view {
                        None
                    } else {
                        Some(View::home())
                    }
                }
                None => {}
            }
        }

        Ok(())
    }
}
