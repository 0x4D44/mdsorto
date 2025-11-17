/////////////////////
/// MD Standup Ordering Randomization TOol - "MDSORTO" (c)(R)(TM)
///
/// 0x4D44 - November 2023
///
/// Takes a list of users, randomly shuffles them & then gives them each a minute to talk.
/// When there's 15 seconds left, the counter goes yellow; it turns red when 5 secs left.
/// - 'space' initially starts & then skips to the next person
/// - 'p' pauses
/// - 'r' restarts on the current person
/// - 'b' goes back to previous person
/// - '+' or '=' gives an extra 10 seconds
/// - '-' or '_' removes 10 seconds
///
pub const APP_VERSION: &str = "MDSORTO V0.3.1";
pub const TICK_INTERVAL_MS: u64 = 60;         // Update tick interval in millisecs
pub const DEFAULT_TIME_EACH: f64 = 60.2;      // Default time each
pub const DEFAULT_PREP_TIME: f64 = 30.2;      // Default initial "prep" time
const CONF_FILE_NAME: &str = "mdsorto.ini";
const PREP_TIME_STR: &str = "Prep time";

// Configuration validation constants
const MIN_TIME: f64 = 1.0;                    // Minimum time in seconds
const MAX_TIME: f64 = 3600.0;                 // Maximum time (1 hour)
const MAX_PEOPLE: usize = 100;                // Maximum number of people

// UI color thresholds
const COLOR_YELLOW_THRESHOLD: f64 = 20.0;     // Yellow warning threshold in seconds
const COLOR_RED_THRESHOLD: f64 = 7.5;         // Red warning threshold in seconds
const MIN_TIME_LEFT: f64 = 0.01;              // Minimum time that can be set

// Time adjustment
const TIME_ADJUSTMENT: f64 = 10.0;            // Seconds to add/remove with +/- keys

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

/// Parse and validate a time configuration value
fn parse_time_config(value: &str, config_name: &str, default: f64) -> f64 {
  match value.parse::<f64>() {
    Ok(time) if time >= MIN_TIME && time <= MAX_TIME => time,
    Ok(time) => {
      warn!("Config value '{}' = {} is out of valid range [{}, {}], using default {}",
            config_name, time, MIN_TIME, MAX_TIME, default);
      eprintln!("Warning: {} value {} out of range, using default {}", config_name, time, default);
      default
    }
    Err(e) => {
      warn!("Failed to parse config value '{}' = '{}': {}, using default {}",
            config_name, value, e, default);
      eprintln!("Warning: Invalid {} value '{}', using default {}", config_name, value, default);
      default
    }
  }
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
    let log_file = File::create("mdsorto.log").unwrap_or_else(|e| {
      eprintln!("Warning: Could not create log file: {}", e);
      eprintln!("Continuing with terminal logging only.");
      // Create a dummy file handle - in practice we'd handle this better
      File::create("/dev/null").expect("Failed to open /dev/null")
    });

    CombinedLogger::init(
      vec![
        TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(LevelFilter::Info, Config::default(), log_file),
      ]
    ).unwrap_or_else(|e| {
      eprintln!("Warning: Could not initialize logger: {}", e);
    });

    info!("Logging for {} initialized (tick interval: {}ms)", APP_VERSION, TICK_INTERVAL_MS);

    // Load config from ini file
    info!("Reading config from {}", CONF_FILE_NAME);
    let inimap = match ini!(safe CONF_FILE_NAME) {
      Ok(map) => map,
      Err(error) => {
        eprintln!("Warning: Couldn't load config file '{}': {}", CONF_FILE_NAME, error);
        eprintln!("Continuing with default values.");
        info!("Using default configuration");
        std::collections::HashMap::new()
      }
    };

    // List all the config
    for (key, value) in &inimap {
      info!("{} / {:?}", key, value);
    }

    info!("Checking for key 'mdsorto/timeeach'");
    if inimap.contains_key("mdsorto") {
      if let Some(section) = inimap.get("mdsorto") {
        // Parse timeeach with validation
        if let Some(val) = section.get("timeeach").and_then(|v| v.as_ref()) {
          info!("Found timeeach config: {}", val);
          self.time_each = parse_time_config(val, "timeeach", DEFAULT_TIME_EACH);
          self.time_left = self.time_each;
          info!("Set time_each to: {} seconds", self.time_each);
        }

        // Parse preptime with validation
        if let Some(val) = section.get("preptime").and_then(|v| v.as_ref()) {
          info!("Found preptime config: {}", val);
          self.prep_time = parse_time_config(val, "preptime", DEFAULT_PREP_TIME);
          self.time_left = self.prep_time;
          info!("Set prep_time to: {} seconds", self.prep_time);
        }

        // Parse people list with validation
        if let Some(val) = section.get("people").and_then(|v| v.as_ref()) {
          info!("Found people config: {}", val);
          self.people.clear();
          self.people.push(PREP_TIME_STR.to_string());

          let names: Vec<String> = val.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .take(MAX_PEOPLE)
            .collect();

          if names.is_empty() {
            warn!("People list is empty, using defaults");
            eprintln!("Warning: No valid names in people list, using defaults");
          } else {
            self.people.extend(names);
            info!("Loaded {} people", self.people.len() - 1);
          }
        }
      }

      // Ensure we have at least some people
      if self.people.len() <= 1 {
        info!("No people configured, adding defaults");
        if self.people.is_empty() {
          self.people.push(PREP_TIME_STR.to_string());
        }
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
    self.time_left += TIME_ADJUSTMENT;
  }

  fn lose10(&mut self) {
    self.time_left -= TIME_ADJUSTMENT;
    if self.time_left < MIN_TIME_LEFT {
      self.time_left = MIN_TIME_LEFT;
    }
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
    let people_count = self.people.len() as u32;
    if self.current_person < people_count {
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
    tui_big_text::BigTextBuilder::default()
      .lines(lines)
      .style(style)
      .build()
  }
    
  fn timer_paragraph(&mut self) -> BigText<'_> {
    let mut style = Style::new().gray();
    if self.state.is_running() {
      if self.time_left > COLOR_YELLOW_THRESHOLD {
        style = Style::new().green();
      } else if self.time_left > COLOR_RED_THRESHOLD {
        style = Style::new().yellow();
      } else {
        style = Style::new().red();
      }
    }
    let duration = self.format_timeleft();
    let lines = vec![duration.into()];
    tui_big_text::BigTextBuilder::default()
      .lines(lines)
      .style(style)
      .build()
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
                    if let Err(e) = _event_tx.send(Event::Key(key)) {
                      log::error!("Failed to send key event: {}", e);
                    }
                }
              }
              Some(Ok(_)) => { }
              Some(Err(_)) => {
                if let Err(e) = _event_tx.send(Event::Error) {
                  log::error!("Failed to send error event: {}", e);
                }
              }
              None => {},
            }
          },
          _ = delay => {
              if let Err(e) = _event_tx.send(Event::Tick) {
                log::error!("Failed to send tick event: {}", e);
              }
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
    if let Err(e) = self.exit() {
      eprintln!("Error during cleanup: {}", e);
      // Don't panic in Drop - just log the error
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_default_app_creation() {
    let app = CountdownApp::new();
    assert_eq!(app.state, AppState::Paused);
    assert_eq!(app.current_person, 0);
    assert_eq!(app.time_each, DEFAULT_TIME_EACH);
    assert_eq!(app.prep_time, DEFAULT_PREP_TIME);
    assert!(app.people.len() >= 1); // At least prep time entry
  }

  #[test]
  fn test_extra10() {
    let mut app = CountdownApp::new();
    app.time_left = 30.0;
    app.extra10();
    assert_eq!(app.time_left, 30.0 + TIME_ADJUSTMENT);
  }

  #[test]
  fn test_lose10() {
    let mut app = CountdownApp::new();
    app.time_left = 30.0;
    app.lose10();
    assert_eq!(app.time_left, 30.0 - TIME_ADJUSTMENT);
  }

  #[test]
  fn test_lose10_minimum() {
    let mut app = CountdownApp::new();
    app.time_left = 5.0;
    app.lose10(); // 5 - 10 = -5, should clamp to MIN_TIME_LEFT
    assert_eq!(app.time_left, MIN_TIME_LEFT);
  }

  #[test]
  fn test_back_at_start() {
    let mut app = CountdownApp::new();
    app.current_person = 0;
    let initial = app.current_person;
    app.back();
    assert_eq!(app.current_person, initial); // Should not go negative
  }

  #[test]
  fn test_back_from_second_person() {
    let mut app = CountdownApp::new();
    app.current_person = 2;
    app.time_left = 10.0;
    app.back();
    assert_eq!(app.current_person, 1);
    assert_eq!(app.time_left, app.time_each); // Should reset to time_each
  }

  #[test]
  fn test_back_to_prep_time() {
    let mut app = CountdownApp::new();
    app.current_person = 1;
    app.time_left = 10.0;
    app.back();
    assert_eq!(app.current_person, 0);
    assert_eq!(app.time_left, app.prep_time); // Should reset to prep_time
  }

  #[test]
  fn test_next_person_advances() {
    let mut app = CountdownApp::new();
    let initial = app.current_person;
    app.next_person();
    assert_eq!(app.current_person, initial + 1);
    assert_eq!(app.time_left, app.time_each);
  }

  #[test]
  fn test_next_person_quits_at_end() {
    let mut app = CountdownApp::new();
    app.current_person = (app.people.len() - 1) as u32;
    app.next_person();
    assert_eq!(app.state, AppState::Quitting);
  }

  #[test]
  fn test_restart() {
    let mut app = CountdownApp::new();
    app.time_left = 10.0;
    app.restart();
    assert_eq!(app.time_left, app.time_each);
  }

  #[test]
  fn test_pause_unpause() {
    let mut app = CountdownApp::new();
    assert_eq!(app.state, AppState::Paused);

    app.unpause();
    assert_eq!(app.state, AppState::Running);

    app.pause();
    assert_eq!(app.state, AppState::Paused);
  }

  #[test]
  fn test_pausetoggle() {
    let mut app = CountdownApp::new();
    assert_eq!(app.state, AppState::Paused);

    app.pausetoggle();
    assert_eq!(app.state, AppState::Running);

    app.pausetoggle();
    assert_eq!(app.state, AppState::Paused);
  }

  #[test]
  fn test_quit() {
    let mut app = CountdownApp::new();
    app.quit();
    assert_eq!(app.state, AppState::Quitting);
  }

  #[test]
  fn test_start_or_next_when_paused() {
    let mut app = CountdownApp::new();
    assert_eq!(app.state, AppState::Paused);
    app.start_or_next();
    assert_eq!(app.state, AppState::Running);
  }

  #[test]
  fn test_start_or_next_when_running() {
    let mut app = CountdownApp::new();
    app.state = AppState::Running;
    let initial_person = app.current_person;
    app.start_or_next();
    assert_eq!(app.current_person, initial_person + 1);
  }

  #[test]
  fn test_format_timeleft() {
    let mut app = CountdownApp::new();

    app.time_left = 90.5;
    assert_eq!(app.format_timeleft(), "01:30.5");

    app.time_left = 125.3;
    assert_eq!(app.format_timeleft(), "02:05.3");

    app.time_left = 5.7;
    assert_eq!(app.format_timeleft(), "00:05.7");
  }

  #[test]
  fn test_parse_time_config_valid() {
    let result = parse_time_config("45.5", "test", DEFAULT_TIME_EACH);
    assert_eq!(result, 45.5);
  }

  #[test]
  fn test_parse_time_config_too_low() {
    let result = parse_time_config("0.5", "test", DEFAULT_TIME_EACH);
    assert_eq!(result, DEFAULT_TIME_EACH); // Should use default
  }

  #[test]
  fn test_parse_time_config_too_high() {
    let result = parse_time_config("5000.0", "test", DEFAULT_TIME_EACH);
    assert_eq!(result, DEFAULT_TIME_EACH); // Should use default
  }

  #[test]
  fn test_parse_time_config_invalid() {
    let result = parse_time_config("not_a_number", "test", DEFAULT_TIME_EACH);
    assert_eq!(result, DEFAULT_TIME_EACH); // Should use default
  }

  #[test]
  fn test_parse_time_config_negative() {
    let result = parse_time_config("-10.0", "test", DEFAULT_TIME_EACH);
    assert_eq!(result, DEFAULT_TIME_EACH); // Should use default
  }

  #[test]
  fn test_parse_time_config_at_min_boundary() {
    let result = parse_time_config("1.0", "test", DEFAULT_TIME_EACH);
    assert_eq!(result, 1.0); // MIN_TIME, should be accepted
  }

  #[test]
  fn test_parse_time_config_at_max_boundary() {
    let result = parse_time_config("3600.0", "test", DEFAULT_TIME_EACH);
    assert_eq!(result, 3600.0); // MAX_TIME, should be accepted
  }
}
