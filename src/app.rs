use crate::config::Config;
use crate::errors::*;
use crate::event::Event;
use crate::rebuilder::{self, Rebuilder, Selectable};
use crossterm::event::EventStream;
use ratatui::{DefaultTerminal, widgets::ListState};

#[derive(Debug)]
pub struct App {
    running: bool,
    pub confirm: bool,
    pub scroll: ListState,
    pub config: Config,
    pub rebuilders: Vec<Selectable<Rebuilder>>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let mut app = Self {
            running: true,
            confirm: false,
            scroll: ListState::default(),
            config,
            rebuilders: vec![],
        };
        app.rebuilders = app.config.resolve_rebuilder_view();
        app
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut events = EventStream::new();

        while self.running {
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
                    self.scroll.select_previous();
                }
                Some(Event::ScrollDown) => {
                    self.scroll.select_next();
                }
                Some(Event::ScrollFirst) => {
                    self.scroll.select_first();
                }
                Some(Event::ScrollLast) => {
                    self.scroll.select_last();
                }
                Some(Event::Reload) => {
                    let list = rebuilder::fetch_rebuilderd_community().await?;
                    self.config.cached_rebuilderd_community = list;
                    self.config.save().await?;

                    self.rebuilders = self.config.resolve_rebuilder_view();
                }
                Some(Event::Toggle) => {
                    if let Some(idx) = self.scroll.selected()
                        && let Some(rebuilder) = self.rebuilders.get_mut(idx)
                    {
                        if rebuilder.active {
                            self.config
                                .selected_rebuilders
                                .retain(|r| r.url != rebuilder.item.url);
                        } else {
                            self.config.selected_rebuilders.push(rebuilder.item.clone());
                        }
                        self.config.save().await?;

                        rebuilder.active = !rebuilder.active;
                    }
                }
                Some(Event::Quit) => self.running = false,
                None => {}
            }
        }

        Ok(())
    }
}
