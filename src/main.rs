/////////////////////
/// MD Standup Ordering Randomization TOol - "MDSORTO" (c)(R)(TM)
///
/// mdavidson - November 2023
///
/// Takes a list of users, randomly shuffles them & then gives them each a minute to talk.
/// When there's 15 seconds left, the counter goes yellow; it turns red when 5 secs left.
/// - 'space' initially starts & then skips to the next person
/// - 'p' pauses
/// - 'r' restarts on the current person
/// - 'b' goes back to previous person
/// - 'e' gives an extra 10 seconds
/// - 'x' removes 10 seconds
///
pub const APP_VERSION: &str = "MDSORTO V0.3.1";
pub const TICK_INTERVAL_MS: u64 = 60;         // Update tick interval in millisecs
pub const DEFAULT_TIME_EACH: f64 = 60.2;      // Default time each
pub const DEFAULT_PREP_TIME: f64 = 30.2;  // Default initial "prep" time
const CONF_FILE_NAME: &str = "mdsorto.ini";
const PREP_TIME_STR: &str = "Prep time";

use std::time::{Duration, Instant};
#[macro_use] extern crate log;
extern crate simplelog;
use simplelog::*;
use std::fs::File;
#[macro_use]
extern crate ini;

use color_eyre::eyre::{eyre, Result};
use futures::{FutureExt, StreamExt};
use ratatui::{backend::CrosstermBackend as Backend, prelude::*, widgets::*};
use strum::EnumIs;
use tui_big_text::BigText;
use crossterm::event::{KeyEvent, KeyCode};
use rand::{thread_rng, seq::SliceRandom};
use build_time::{build_time_local};


macro_rules! vec_of_strings {
  ($($x:expr),*) => (vec![$($x.to_string()),*]);
}


#[derive(Clone, Debug)]
pub enum Event {
  Error,
  Tick,
  Key(KeyEvent),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, EnumIs)]
enum AppState {
  #[default]
  Paused,
  Running,
  Quitting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Message {
  StartOrNext,
  Pause,
  Restart,
  Back,
  Extra10,
  Lose10,
  Tick,
  Quit,
}

#[tokio::main]
async fn main() -> Result<()> {
  let mut app = CountdownApp::default();
  app.run().await
}

#[derive(Debug, Clone, PartialEq)]
struct CountdownApp<> {
  state: AppState,
  people: Vec<String>,
  current_person: u32,
  last_tick_time: Instant,
  prep_time: f64,
  time_each: f64,
  time_left: f64,
}
impl Default for CountdownApp<> {
  fn default() -> Self {
    Self::new()
  }
}
impl CountdownApp<> {
  fn new() -> Self {
    Self {
      state: Default::default(),
      people: vec_of_strings![PREP_TIME_STR, "Person 1", "Person 2", "Person 3"],
      current_person: 0,
      last_tick_time: Instant::now(),
      prep_time: DEFAULT_PREP_TIME,
      time_each: DEFAULT_TIME_EACH,
      time_left: DEFAULT_TIME_EACH,
    }
  }

  async fn run(&mut self) -> Result<()> {
    // Init logging
    CombinedLogger::init(
      vec![
        TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(LevelFilter::Info, Config::default(), File::create("mdsorto.log").unwrap()),
      ]
    ).unwrap();
  
    info!("Logging for {} initialized (tick interval: {}ms)", APP_VERSION, TICK_INTERVAL_MS);

    // Load config from ini file
    info!("Reading config from {}", CONF_FILE_NAME);
    let inimap = ini!(safe CONF_FILE_NAME).unwrap_or_else(|error| {
      eprintln!("*** Couldn't load config file {} ({}) ***\n", CONF_FILE_NAME, APP_VERSION);
      panic!("Error: {}", error);
    });

    // List all the config
    for (key, value) in &inimap {
      info!("{} / {:?}", key, value);
    }

    info!("Checking for key 'mdsorto/timeeach'");
    if inimap.contains_key("mdsorto") {
      if inimap["mdsorto"].contains_key("timeeach") {
        let val = inimap["mdsorto"]["timeeach"].clone().unwrap();
        info!("Overriding default time each to: {} seconds", val);
        self.time_each = val.parse::<f64>().unwrap();
        self.time_left = self.time_each;
      }

      if inimap["mdsorto"].contains_key("preptime") {
        let val = inimap["mdsorto"]["preptime"].clone().unwrap();
        info!("Overriding default prep time to: {} seconds", val);
        self.prep_time = val.parse::<f64>().unwrap();
        self.time_left = self.prep_time;
      }

      if inimap["mdsorto"].contains_key("people") {
        self.people.clear();
        self.people.push(PREP_TIME_STR.to_string());
        
        let val = inimap["mdsorto"]["people"].clone().unwrap();
        let names = val.split(",");
        for name in names {
          self.people.push(name.trim().to_string());
        }
      }

      if self.people.is_empty() {
        // Can't have a standup with no people, so add two people back in
        self.people.push(PREP_TIME_STR.to_string());
        self.people.push("Person 1".to_string());
        self.people.push("Person 2".to_string());
      }
    }
    
    // Shuffle the order of people
    self.people[1..].shuffle(&mut thread_rng());

    // Ratatui main loop
    let mut tui = Tui::new()?;
    tui.enter()?;
    while !self.state.is_quitting() {
      tui.draw(|f| self.ui(f).expect("Unexpected error during drawing"))?;
      let event = tui.next().await.ok_or(eyre!("Unable to get event"))?; // blocks until next event
      let message = self.handle_event(event)?;
      self.update(message)?;
    }
    tui.exit()?;
    println!("Thanks for using {} (built: {})\n", APP_VERSION, build_time_local!("%Y-%b-%d at %H:%M:%S"));
    Ok(())
  }

  // Event handler (keyboard, tick)
  fn handle_event(&self, event: Event) -> Result<Message> {
    let msg = match event {
      Event::Key(key) => {
        match key.code {
          KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Message::Quit,
          KeyCode::Char('p') | KeyCode::Char('P') => Message::Pause,
          KeyCode::Char(' ') | KeyCode::Enter => Message::StartOrNext,
          KeyCode::Char('b') | KeyCode::Char('B') => Message::Back,
          KeyCode::Char('r') | KeyCode::Char('R') => Message::Restart,
          KeyCode::Char('=') | KeyCode::Char('+') => Message::Extra10,
          KeyCode::Char('-') | KeyCode::Char('_') => Message::Lose10,
          _ => Message::Tick,
        }
      },
      _ => Message::Tick,
    };
    Ok(msg)
  }

  fn update(&mut self, message: Message) -> Result<()> {
    match message {
      Message::StartOrNext => self.start_or_next(),
      Message::Pause => self.pausetoggle(),
      Message::Back => self.back(),
      Message::Extra10 => self.extra10(),
      Message::Lose10 => self.lose10(),
      Message::Restart => self.restart(),
      Message::Tick => self.tick(),
      Message::Quit => self.quit(),
    }
    Ok(())
  }

  fn start_or_next(&mut self) {
    if self.state.is_paused() { 
      self.unpause();
      self.last_tick_time = Instant::now();
    } 
    else if self.state.is_running() {
      self.next_person();
    }
  }

  fn pausetoggle(&mut self) {
    if self.state.is_paused() { self.unpause() } else {self.pause() };
  }

  fn pause(&mut self) {
    self.state = AppState::Paused;
  }

  fn unpause(&mut self) {
    self.state = AppState::Running;
  }

  fn back(&mut self) {
    if self.current_person == 0 { return };

    self.current_person -= 1;
    if self.current_person == 0 {
      self.time_left = self.prep_time;
    }
    else{
      self.time_left = self.time_each;
    }
  }

  fn extra10(&mut self) {
    self.time_left += 10.0;
  }

  fn lose10(&mut self) {
    self.time_left -= 10.0;
    if self.time_left < 0.01 { self.time_left = 0.01 };
  }

  fn restart(&mut self) {
    self.time_left = self.time_each;
  }

  fn tick(&mut self) {
    let now = Instant::now();
    let dur = now - self.last_tick_time;
    self.last_tick_time = now;
    if !self.state.is_paused() { self.time_left -= dur.as_secs_f64() };

    if self.time_left <= 0.0 {
      // Time for the next person
      self.next_person();
    }
  }

  fn next_person(&mut self) {
    self.current_person += 1;
    if self.current_person < self.people.len().try_into().unwrap() {
      self.time_left = self.time_each;
    } else {
      self.quit();
    }
  }

  fn quit(&mut self) {
    self.state = AppState::Quitting;
  }

  fn ui(&mut self, f: &mut Frame) -> Result<()> {
    let layout = self.layout(f.size());
    f.render_widget(self.title_paragraph(), layout[0]);
    f.render_widget(self.person_paragraph(), layout[1]);
    f.render_widget(self.timer_paragraph(), layout[2]);
    f.render_widget(self.people_list(), layout[3]);
    f.render_widget(self.help_paragraph(), layout[4]);
    Ok(())
  }

  fn layout(&self, area: Rect) -> Vec<Rect> {
    let layout = Layout::default()
      .direction(Direction::Vertical)
      .constraints(vec![
        Constraint::Length(3), // top bar
        Constraint::Length(9), // person
        Constraint::Length(9), // timer
        Constraint::Length(2), // list of people
        Constraint::Length(2), // help
      ])
      .split(area);

    // Returns a vector of rectangles for the layout
    layout.to_vec()
  }

  fn title_paragraph(&mut self) -> Paragraph<'_> {
    let title_text =
      Line::from(vec![APP_VERSION.into(), " - S".into(), "tandup".dim(), " O".into(), 
        "rdering".dim(), " R".into(), "andomization".dim(), " TO".into(), "ol".dim()]);
    Paragraph::new(title_text).gray()
  }

  fn person_paragraph(&mut self) -> BigText<'_> {
    let mut style = Style::new().gray();
    if self.current_person == 0 {
      style = Style::new().blue();
    }
    let person: &str = &self.people[self.current_person as usize];
    let lines = vec![person.into()];
    tui_big_text::BigTextBuilder::default().lines(lines).style(style).build().unwrap()
  }
    
  fn timer_paragraph(&mut self) -> BigText<'_> {
    let mut style = Style::new().gray();
    if self.state.is_running() { 
      if self.time_left > 20.0 { style = Style::new().green() } 
        else if self.time_left > 7.5 { style = Style::new().yellow() } 
        else { style = Style::new().red() };
    };
    let duration = self.format_timeleft();
    let lines = vec![duration.into()];
    tui_big_text::BigTextBuilder::default().lines(lines).style(style).build().unwrap()
  }

  fn people_list(&mut self) -> Paragraph<'_> {
    let people_list_text: String = self.people[1..].iter().map(|x| format!("{} ", x)).collect();
    Paragraph::new(people_list_text).gray()
  }
  
  fn help_paragraph(&mut self) -> Paragraph<'_> {
    let mut space_action = if self.state.is_running() { "next"} else { "start" };
    if (self.current_person + 1) == self.people.len() as u32 { space_action = "quit"};
    let pause_action = if self.state.is_paused() { "unpause" } else { "pause" };
    let prev_action1 = if self.current_person == 0 { "" } else { " : b " };
    let prev_action2 = if self.current_person == 0 { "" } else { "previous" };
    let help_text =
      Line::from(vec!["space ".into(), space_action.dim(), prev_action1.into(), prev_action2.dim(),
        " : p ".into(), pause_action.dim(), " : q ".into(), "quit".dim(), " : r ".into(), 
        "restart".dim(), " : + ".into(), "+10s".dim(), " : - ".into(), "-10s".dim()]);
    Paragraph::new(help_text).gray()
  }

  fn format_timeleft(&self) -> String {
    format!("{:02}:{:02}.{:01}", 
      (self.time_left as u32) / 60, 
      (self.time_left as u32) % 60, 
      ((self.time_left*10.0) as u32) % 10)
  }
}

struct Tui {
  pub terminal: Terminal<Backend<std::io::Stderr>>,
  pub task: tokio::task::JoinHandle<()>,
  pub cancellation_token: tokio_util::sync::CancellationToken,
  pub event_rx: tokio::sync::mpsc::UnboundedReceiver<Event>,
  pub event_tx: tokio::sync::mpsc::UnboundedSender<Event>,
}

impl Tui {
  fn new() -> Result<Tui> {
    let mut terminal = ratatui::Terminal::new(Backend::new(std::io::stderr()))?;
    terminal.clear()?;
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let task = tokio::spawn(async {});
    Ok(Self { terminal, task, cancellation_token, event_rx, event_tx })
  }

  pub async fn next(&mut self) -> Option<Event> {
    self.event_rx.recv().await
  }

  pub fn enter(&mut self) -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), crossterm::terminal::EnterAlternateScreen, crossterm::cursor::Hide)?;
    self.start();
    Ok(())
  }

  pub fn exit(&self) -> Result<()> {
    self.stop()?;
    crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen, crossterm::cursor::Show)?;
    crossterm::terminal::disable_raw_mode()?;
    Ok(())
  }

  pub fn cancel(&self) {
    self.cancellation_token.cancel();
  }

  pub fn stop(&self) -> Result<()> {
    self.cancel();
    let mut counter = 0;
    while !self.task.is_finished() {
      std::thread::sleep(Duration::from_millis(250));
      counter += 1;
      if counter > 5 {
        self.task.abort();
      }
      if counter > 10 {
        log::error!("Failed to abort task for unknown reason");
        return Err(eyre!("Unable to abort task"));
      }
    }
    Ok(())
  }

  pub fn start(&mut self) {
    let tick_rate = std::time::Duration::from_millis(TICK_INTERVAL_MS);
    self.cancel();
    self.cancellation_token = tokio_util::sync::CancellationToken::new();
    let _cancellation_token = self.cancellation_token.clone();
    let _event_tx = self.event_tx.clone();
    self.task = tokio::spawn(async move {
      let mut reader = crossterm::event::EventStream::new();
      let mut interval = tokio::time::interval(tick_rate);
      loop {
        let delay = interval.tick();
        let crossterm_event = reader.next().fuse();
        tokio::select! {
          _ = _cancellation_token.cancelled() => {
            break;
          }
          maybe_event = crossterm_event => {
            match maybe_event {
              Some(Ok(crossterm::event::Event::Key(key))) => {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    _event_tx.send(Event::Key(key)).unwrap();
                }
              }
              Some(Ok(_)) => { }
              Some(Err(_)) => {
                _event_tx.send(Event::Error).unwrap();
              }
              None => {},
            }
          },
          _ = delay => {
              _event_tx.send(Event::Tick).unwrap();
          },
        }
      }
    });
  }
}

impl std::ops::Deref for Tui {
  type Target = ratatui::Terminal<Backend<std::io::Stderr>>;

  fn deref(&self) -> &Self::Target {
    &self.terminal
  }
}

impl std::ops::DerefMut for Tui {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.terminal
  }
}

impl Drop for Tui {
  fn drop(&mut self) {
    self.exit().unwrap();
  }
}
