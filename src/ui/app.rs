use crate::db::desktop_entry::DesktopEntryEntity;
use crate::db::plugin::PluginCommandEntity;
use crate::entries::pop_entry::PopResponse;
use crate::entries::{AsEntry};
use crate::freedesktop::desktop::DesktopEntry;
use crate::ui::mode::ActiveMode;
use crate::ui::state::{Selection, State};
use crate::ui::subscriptions::pop_launcher::{PopLauncherSubscription, SubscriptionMessage};
use crate::{THEME};
use iced::futures::channel::mpsc::{Sender, TrySendError};
use iced::keyboard::KeyCode;
use iced::{Alignment, Application, Color, Column, Container, Element, Length, Padding, Row, Scrollable, TextInput, Text};
use iced_native::{Command, Event, Subscription};
use log::debug;
use pop_launcher::Request;
use pop_launcher::Request::Activate;
use std::path::Path;
use std::process::exit;
use crate::db::web::WebEntity;
use crate::ui::plugin_matchers::Plugin;
use crate::ui::style::search::ModeHint;
use crate::ui::subscriptions::plugin_configs::PluginMatcherSubscription;

#[derive(Debug)]
pub struct Onagre {
    state: State,
    request_tx: Option<Sender<Request>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    KeyboardEvent(KeyCode),
    SubscriptionResponse(SubscriptionMessage),
    PluginConfig(Plugin),
    Unfocused,
}

impl Application for Onagre {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_: Self::Flags) -> (Self, Command<Self::Message>) {
        let onagre = Onagre {
            state: Default::default(),
            request_tx: Default::default(),
        };

        (onagre, Command::none())
    }

    fn title(&self) -> String {
        "Onagre".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        self.state.input.focus();

        match message {
            Message::InputChanged(input) => self.on_input_changed(input),
            Message::KeyboardEvent(event) => self.handle_input(event),
            Message::SubscriptionResponse(message) => self.on_pop_launcher_message(message),
            Message::Unfocused => exit(0),
            Message::PluginConfig(plugin) => {
                self.state.plugin_matchers.insert(plugin.name.clone(), plugin);
                Command::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let keyboard_event = Onagre::keyboard_event();
        let pop_launcher = PopLauncherSubscription::create().map(Message::SubscriptionResponse);
        let matchers = PluginMatcherSubscription::create().map(Message::PluginConfig);
        let subs = vec![keyboard_event, pop_launcher, matchers];
        Subscription::batch(subs)
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        // Build rows from current mode search entries
        let selected = self.selected();
        let rows = match &self.state.get_active_mode() {
            ActiveMode::Plugin { plugin_name, history, .. } if *history =>
                self.state.cache.plugin_history(plugin_name)
                    .iter()
                    .enumerate()
                    .map(|(idx, entry)| entry.to_row(selected, idx).into())
                    .collect(),
            ActiveMode::Web(web_name) =>
                self.state.cache.web_history(web_name)
                    .iter()
                    .enumerate()
                    .map(|(idx, entry)| entry.to_row(selected, idx).into())
                    .collect(),
            ActiveMode::History =>
                self.state
                    .cache
                    .de_history()
                    .iter()
                    .enumerate()
                    .map(|(idx, entry)| entry.to_row(selected, idx).into())
                    .collect(),
            _ =>
                self.state
                    .pop_search
                    .iter()
                    .map(|entry| entry.to_row(selected, entry.id as usize).into())
                    .collect(),
        };

        let entries_column = Column::with_children(rows);

        // Scrollable element containing the rows
        let scrollable = Container::new(
            Scrollable::new(&mut self.state.scroll)
                .push(entries_column)
                .height(THEME.scrollable.height.into())
                .width(THEME.scrollable.width.into())
                .scrollbar_width(THEME.scrollable.scroller_width)
                .scroller_width(THEME.scrollable.scrollbar_width)
                .style(&THEME.scrollable),
        )
            .style(&THEME.scrollable)
            .padding(THEME.scrollable.padding);

        let mode_hint = Container::new(Row::new()
            .push(Text::new(&self.state.input_value.modifier_display)))
            .style(ModeHint);


        let search_input = TextInput::new(
            &mut self.state.input,
            "Search",
            &self.state.input_value.input_display,
            Message::InputChanged,
        )
            .style(&THEME.search.bar)
            .padding(Padding {
                top: 0,
                right: 0,
                bottom: 0,
                left: 10,
            })
            .width(THEME.search.bar.text_width.into());

        let search_bar = Container::new(
            Row::new()
                .spacing(20)
                .align_items(Alignment::Center)
                .padding(2)
                .push(mode_hint)
                .push(search_input)
                .width(THEME.search.width.into())
                .height(THEME.search.height.into()),
        )
            .padding(THEME.search.padding)
            .style(&THEME.search);

        let app_container = Container::new(
            Column::new()
                .push(search_bar)
                .push(scrollable)
                .align_items(Alignment::Start)
                .height(Length::Fill)
                .width(Length::Fill)
                .padding(20),
        )
            .height(Length::Fill)
            .width(Length::Fill)
            .style(THEME.as_ref());

        app_container.into()
    }

    fn background_color(&self) -> Color {
        Color::TRANSPARENT
    }
}

impl Onagre {
    // Only call this if we are using entries from the database
    // in order to re-ask pop-launcher for the exact same entry
    fn current_entry(&self) -> Option<String> {
        let selected = self.selected();
        match &self.state.get_active_mode() {
            ActiveMode::History => self
                .state
                .cache
                .de_history()
                .get(selected.unwrap())
                .map(|entry| entry.path.to_string_lossy().to_string()),
            ActiveMode::Plugin {
                modifier, plugin_name, ..
            } => {
                // Get user input as pop-entry
                match selected {
                    None => {
                        return self
                            .state
                            .pop_search
                            .get(0)
                            .map(|entry| entry.name.clone());
                    }
                    Some(selected) => self
                        .state
                        .cache
                        .plugin_history(plugin_name)
                        .get(selected)
                        .map(|entry| format!("{}{}", modifier, entry.query)),
                }
            }
            ActiveMode::Web(web_name) => {
                // Get user input as pop-entry
                match selected {
                    None => {
                        return self
                            .state
                            .pop_search
                            .get(0)
                            .map(|entry| entry.name.clone());
                    }
                    Some(selected) => self
                        .state
                        .cache
                        .web_history(web_name)
                        .get(selected)
                        .map(|entry| entry.query()),
                }
            }
            _pop_mode => None,
        }
    }

    fn on_input_changed(&mut self, input: String) -> Command<Message> {
        self.state.set_input(&input);
        self.state.selected = match self.state.get_active_mode() {
            // For those mode first line is unselected on change
            // We want to issue a pop-launcher search request to get the query at index 0 in
            // the next search response, then activate it
            ActiveMode::Web(_) | ActiveMode::History => Selection::Reset,
            ActiveMode::Plugin { history, .. } if *history => Selection::Reset,
            _ => Selection::PopLauncher(0),
        };

        self.state.scroll.snap_to(0.0);

        match &self.state.get_active_mode() {
            ActiveMode::History => {}
            _ => {
                let value = self.state.get_input();
                self.pop_request(Request::Search(value))
                    .expect("Unable to send search request to pop-launcher")
            }
        }

        Command::none()
    }

    fn run_command<P: AsRef<Path>>(&self, desktop_entry_path: P) -> Command<Message> {
        let desktop_entry = DesktopEntry::from_path(&desktop_entry_path).unwrap();

        DesktopEntryEntity::persist(&desktop_entry, desktop_entry_path.as_ref(), &self.state.cache.db);

        let argv = shell_words::split(&desktop_entry.exec);
        let args = argv.unwrap();
        let args = args
            .iter()
            // Filter out special freedesktop syntax
            .filter(|entry| !entry.starts_with('%'))
            .collect::<Vec<&String>>();

        std::process::Command::new(&args[0])
            .args(&args[1..])
            .spawn()
            .expect("Command failure");

        exit(0);
    }

    fn handle_input(&mut self, key_code: KeyCode) -> Command<Message> {
        match key_code {
            KeyCode::Up => {
                self.dec_selected();
                self.snap();
                debug!("Selected line : {:?}", self.selected());
            }
            KeyCode::Down => {
                self.inc_selected();
                debug!("Selected line : {:?}", self.selected());
            }
            KeyCode::Enter => return self.on_execute(),
            KeyCode::Tab => {
                if let Some(selected) = self.selected() {
                    self.pop_request(Request::Complete(selected as u32))
                        .expect("Unable to send request to pop-launcher");
                }
            }
            KeyCode::Escape => {
                exit(0);
            }
            _ => {}
        };

        Command::none()
    }

    fn snap(&mut self) {
        let total_items = self.current_entries_len() as f32;
        match self.selected() {
            None => self.state.scroll.snap_to(0.0),
            Some(selected) => {
                let line_offset = if selected == 0 { 0 } else { &selected + 1 } as f32;

                let offset = (1.0 / total_items) * (line_offset) as f32;
                self.state.scroll.snap_to(offset);
            }
        }
    }

    fn on_pop_launcher_message(&mut self, message: SubscriptionMessage) -> Command<Message> {
        match message {
            SubscriptionMessage::Ready(sender) => {
                self.request_tx = Some(sender);
            }
            SubscriptionMessage::PopMessage(response) => match response {
                PopResponse::Close => exit(0),
                PopResponse::Context { .. } => todo!("Discrete graphics is not implemented"),
                PopResponse::DesktopEntry { path, .. } => {
                    debug!("Launch DesktopEntry {path:?} via run_command");
                    self.run_command(path);
                }
                PopResponse::Update(search_updates) => {
                    if self.state.exec_on_next_search {
                        debug!("Launch entry 0 via PopRequest::Activate");
                        self.pop_request(Activate(0))
                            .expect("Unable to send Activate request to pop-launcher");
                        return Command::none();
                    }
                    self.state.pop_search = search_updates;
                }
                PopResponse::Fill(fill) => {
                    // Fixme: we can probably avoid cloning here
                    let mode_prefix = &self.state.input_value.modifier_display;
                    let fill = fill.strip_prefix(mode_prefix)
                        .expect("Auto-completion Error");
                    self.state.input_value.input_display = fill.into();
                    self.state.input.move_cursor_to_end();
                    let filled = self.state.input_value.input_display.clone();
                    self.on_input_changed(filled);
                }
            },
        };

        Command::none()
    }

    fn on_execute(&mut self) -> Command<Message> {
        match &self.state.get_active_mode() {
            ActiveMode::Plugin { plugin_name, history, .. } if *history => {
                PluginCommandEntity::persist(plugin_name, &self.state.get_input(), &self.state.cache.db);

                // Running the user input query at index zero
                if self.selected().is_none() {
                    self.pop_request(Activate(0))
                        .expect("Unable to send pop-launcher request")
                } else {
                    // Re ask pop-launcher for a stored query
                    self.state.exec_on_next_search = true;
                    let command = self.current_entry().unwrap();
                    self.state.set_input(&command);
                    self.pop_request(Request::Search(command))
                        .expect("Unable to send pop-launcher request");
                }
            }
            ActiveMode::Web(kind) => {
                let query = self.state.get_input();
                let query = query.strip_prefix(kind).unwrap();
                WebEntity::persist(query, kind, &self.state.cache.db);
                // Running the user input query at index zero
                if self.selected().is_none() {
                    self.pop_request(Activate(0))
                        .expect("Unable to send pop-launcher request")
                } else {
                    // Re ask pop-launcher for a stored query
                    let command = self.current_entry().unwrap();
                    self.state.set_input(&command);
                    self.state.exec_on_next_search = true;
                    self.pop_request(Request::Search(command))
                        .expect("Unable to send pop-launcher request")
                }
            }
            ActiveMode::History => {
                let path = self.current_entry();
                let path = path.unwrap();
                self.run_command(path);
            }
            _ => {
                if self.selected().is_none() {
                    self.pop_request(Activate(0))
                        .expect("Unable to send pop-launcher request")
                } else {
                    let selected = self.selected().unwrap() as u32;
                    debug!("Activating pop entry at index {selected}");
                    self.pop_request(Activate(selected))
                        .expect("Unable to send pop-launcher request")
                }
            }
        }

        Command::none()
    }

    fn current_entries_len(&self) -> usize {
        match &self.state.get_active_mode() {
            ActiveMode::Plugin { plugin_name, history, .. } => if *history {
                self.state.cache.plugin_history_len(plugin_name)
            } else {
                self.state.pop_search.len()
            },
            ActiveMode::History => self.state.cache.de_len(),
            ActiveMode::DesktopEntry => self.state.pop_search.len(),
            ActiveMode::Web(web_name) => self.state.cache.web_history_len(web_name)
        }
    }

    fn pop_request(&self, request: Request) -> Result<(), TrySendError<Request>> {
        let sender = self.request_tx.as_ref().unwrap();
        let mut sender = sender.clone();
        debug!("Sending message to pop launcher : {:?}", request);
        sender.try_send(request)
    }

    fn selected(&self) -> Option<usize> {
        match self.state.selected {
            Selection::Reset => None,
            Selection::History(idx) | Selection::PopLauncher(idx) => Some(idx)
        }
    }

    fn dec_selected(&mut self) {
        match self.state.selected {
            Selection::Reset => self.state.selected = Selection::Reset,
            Selection::History(selected) => {
                if selected > 0 {
                    self.state.selected = Selection::History(selected - 1)
                }
            }
            Selection::PopLauncher(selected) => {
                if selected > 0 {
                    self.state.selected = Selection::PopLauncher(selected - 1)
                }
            }
        };
    }

    fn inc_selected(&mut self) {
        match self.state.selected {
            Selection::Reset => self.state.selected = Selection::History(0),
            Selection::History(selected) => {
                let total_items = self.current_entries_len();
                if total_items != 0 && selected < total_items - 1 {
                    self.state.selected = Selection::History(selected + 1);
                    self.snap();
                }
            }
            Selection::PopLauncher(selected) => {
                let total_items = self.current_entries_len();
                if total_items != 0 && selected < total_items - 1 {
                    self.state.selected = Selection::PopLauncher(selected + 1);
                    self.snap();
                }
            }
        };
    }

    fn keyboard_event() -> Subscription<Message> {
        iced_native::subscription::events_with(|event, _status| match event {
            Event::Window(iced_native::window::Event::Unfocused) => Some(Message::Unfocused),
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                                modifiers: _,
                                key_code,
                            }) => Some(Message::KeyboardEvent(key_code)),
            _ => None,
        })
    }
}