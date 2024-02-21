use std::error::Error;
use std::fmt::Write;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use rand::Rng;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Alignment;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui::Frame;

const INSPECT_DURATION: Duration = Duration::from_secs(15);
const SCRAMBLE_MOVES: usize = 30;

#[derive(Clone, Debug, Default)]
struct App {
    state: State,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle(Scramble),
    Inspecting(Instant),
    Solving(Instant),
    Done(Duration),
}

impl Default for State {
    fn default() -> Self {
        Self::Idle(Scramble::random())
    }
}

impl State {
    fn is_idle(&self) -> bool {
        matches!(self, State::Idle(_))
    }

    fn next(&mut self) {
        match self {
            Self::Idle(_) => {
                *self = Self::Inspecting(Instant::now());
            }
            Self::Inspecting(_) => {
                *self = State::Solving(Instant::now());
            }
            Self::Solving(start) => {
                let duration = Instant::now().duration_since(*start);
                *self = State::Done(duration);
            }
            State::Done(_) => {
                *self = State::Idle(Scramble::random());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Scramble {
    moves: [Move; SCRAMBLE_MOVES],
}

impl Default for Scramble {
    fn default() -> Self {
        Self::random()
    }
}

impl std::fmt::Display for Scramble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.moves[0])?;
        for mov in self.moves[1..].iter() {
            write!(f, " {}", mov)?;
        }

        Ok(())
    }
}

impl Scramble {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let mut moves = [Move(0); SCRAMBLE_MOVES];
        moves[0] = Move::random(&mut rng);
        for i in 1..SCRAMBLE_MOVES {
            let prev_dir = moves[i - 1].dir();
            moves[i] = Move::random_without(&mut rng, prev_dir);
        }

        Self { moves }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Move(u8);

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.dir() {
            Dir::Front => f.write_char('F')?,
            Dir::Back => f.write_char('B')?,
            Dir::Left => f.write_char('L')?,
            Dir::Right => f.write_char('R')?,
            Dir::Up => f.write_char('U')?,
            Dir::Down => f.write_char('D')?,
        }
        match self.modifier() {
            Mod::Forward => (),
            Mod::Reverse => f.write_char('\'')?,
            Mod::Double => f.write_char('2')?,
        }
        Ok(())
    }
}

impl Move {
    const DOUBLE: u8 = 0x80;
    const REVERSE: u8 = 0x40;

    pub fn random(rng: &mut impl Rng) -> Self {
        let mut mov: u8 = rng.gen_range(0..6);

        let modifier: u8 = rng.gen_range(0..3);
        match modifier {
            0 => (),
            1 => mov |= Self::REVERSE,
            2 | _ => mov |= Self::DOUBLE,
        }

        Self(mov)
    }

    pub fn random_without(rng: &mut impl Rng, dir: Dir) -> Self {
        let mut mov: u8 = rng.gen_range(0..5);
        if mov >= (dir as u8) {
            mov += 1;
        }

        let modifier: u8 = rng.gen_range(0..3);
        match modifier {
            0 => (),
            1 => mov |= Self::REVERSE,
            2 | _ => mov |= Self::DOUBLE,
        }

        Self(mov)
    }

    pub fn dir(&self) -> Dir {
        let dir = self.0 & 0x07;
        // SAFETY: Dir is repr(u8)
        unsafe { std::mem::transmute(dir) }
    }

    pub fn modifier(&self) -> Mod {
        if (self.0 & Self::DOUBLE) != 0 {
            return Mod::Double;
        }
        match (self.0 & Self::REVERSE) != 0 {
            true => Mod::Reverse,
            false => Mod::Forward,
        }
    }
}

#[allow(unused)]
#[repr(u8)]
enum Dir {
    Front = 0x00,
    Back = 0x01,
    Left = 0x02,
    Right = 0x03,
    Up = 0x04,
    Down = 0x05,
}

enum Mod {
    Forward,
    Reverse,
    Double,
}

fn main() {
    if let Err(e) = run() {
        println!("{e}");
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::default();

    let res = loop {
        match input(&mut app) {
            Ok(c) if c == false => break Ok(()),
            Ok(_) => (),
            Err(e) => break Err(e),
        }

        update(&mut app);

        let res = terminal.draw(|frame| ui(&mut app, frame));
        if let Err(e) = res {
            break Err(e.into());
        }
    };

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    res
}

fn input(app: &mut App) -> Result<bool, Box<dyn Error>> {
    if crossterm::event::poll(Duration::from_millis(1))? {
        let event = crossterm::event::read()?;
        if let Event::Key(k) = event {
            if k.kind == KeyEventKind::Press {
                match k.code {
                    KeyCode::Char('q') => return Ok(false),
                    KeyCode::Char('r') if app.state.is_idle() => {
                        app.state = State::Idle(Scramble::random());
                    }
                    KeyCode::Char(' ') => app.state.next(),
                    _ => (),
                }
            }
        }
    }

    Ok(true)
}

fn update(app: &mut App) {
    match app.state {
        State::Idle(_) => (),
        State::Inspecting(start) => {
            let now = Instant::now();
            let duration = now.duration_since(start);
            if duration > INSPECT_DURATION {
                app.state = State::Solving(now);
            }
        }
        State::Solving(_) => (),
        State::Done(_) => (),
    }
}

fn ui(app: &mut App, frame: &mut Frame) {
    let size = frame.size();

    let (text, color) = match app.state {
        State::Idle(scramble) => {
            let text = format!("Press space to start\n{scramble}");
            (text, Color::Yellow)
        }
        State::Inspecting(start) => {
            let duration = Instant::now().duration_since(start);
            let remaining = INSPECT_DURATION.saturating_sub(duration);
            let secs = remaining.as_secs_f32();
            let text = format!("Inspecting\n{secs:.3}s");
            (text, Color::Blue)
        }
        State::Solving(start) => {
            let duration = Instant::now().duration_since(start);
            let secs = duration.as_secs_f32();
            let text = format!("Solving\n{secs:.3}s");
            (text, Color::Green)
        }
        State::Done(duration) => {
            let secs = duration.as_secs_f32();
            let text = format!("Done\n{secs:.3}s");
            (text, Color::Yellow)
        }
    };
    let p = Paragraph::new(text)
        .block(Block::new().padding(Padding::top(size.height / 2)))
        .style(Style::new().fg(color).bold())
        .alignment(Alignment::Center);
    frame.render_widget(p, size);
}
