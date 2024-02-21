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
    color_background: bool,
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
        let mut prev_dirs = PrevDirs(0);
        for mov in &mut moves {
            *mov = Move::random(&mut rng, prev_dirs);
            prev_dirs.update(mov.dir());
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
            Mod::Forward => f.write_char(' ')?,
            Mod::Reverse => f.write_char('\'')?,
            Mod::Double => f.write_char('2')?,
        }
        Ok(())
    }
}

impl Move {
    #[rustfmt::skip]
    const DOUBLE: u8   = 0b1000_0000;
    #[rustfmt::skip]
    const REVERSE: u8  = 0b0100_0000;
    #[rustfmt::skip]
    const DIR_MASK: u8 = 0b0011_1111;

    pub fn random(rng: &mut impl Rng, prev_dirs: PrevDirs) -> Self {
        let mut mov = 0;

        let num_dirs = 6 - prev_dirs.0.count_ones() as u8;
        let mut dir: u8 = rng.gen_range(0..num_dirs);

        for i in 0..6 {
            let bit = 1 << i;
            if !prev_dirs.get(bit) {
                if dir == 0 {
                    mov |= bit;
                    break;
                }

                dir -= 1;
            }
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
        let dir = self.0 & Self::DIR_MASK;
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

#[derive(Clone, Copy)]
struct PrevDirs(u8);

impl PrevDirs {
    fn update(&mut self, dir: Dir) {
        self.0 &= match dir {
            Dir::Front | Dir::Back => Dir::Front as u8 | Dir::Back as u8,
            Dir::Left | Dir::Right => Dir::Left as u8 | Dir::Right as u8,
            Dir::Up | Dir::Down => Dir::Up as u8 | Dir::Down as u8,
        };
        self.0 |= dir as u8;
    }

    fn get(&self, bit: u8) -> bool {
        (self.0 & bit) != 0
    }
}

#[repr(u8)]
#[rustfmt::skip]
enum Dir {
    Front = 1 << 0,
    Back  = 1 << 1,
    Left  = 1 << 2,
    Right = 1 << 3,
    Up    = 1 << 4,
    Down  = 1 << 5,
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
                    KeyCode::Char('c') => {
                        app.color_background = !app.color_background;
                    }
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
            let text = format!("Press space to start\n\n{scramble}");
            (text, Color::Gray)
        }
        State::Inspecting(start) => {
            let duration = Instant::now().duration_since(start);
            let remaining = INSPECT_DURATION.saturating_sub(duration);
            let secs = remaining.as_secs_f32();
            let text = format!("Inspecting\n\n{secs:.3}s");
            (text, Color::Blue)
        }
        State::Solving(start) => {
            let duration = Instant::now().duration_since(start);
            let secs = duration.as_secs_f32();
            let text = format!("Solving\n\n{secs:.3}s");
            (text, Color::Green)
        }
        State::Done(duration) => {
            let secs = duration.as_secs_f32();
            let text = format!("Done\n\n{secs:.3}s");
            (text, Color::Magenta)
        }
    };

    let mut block = Block::new().padding(Padding::top((size.height / 2).saturating_sub(1)));
    if app.color_background {
        block = block.style(Style::new().bg(color));
    }

    let fg_color = if app.color_background {
        Color::Black
    } else {
        color
    };

    let p = Paragraph::new(text)
        .block(block)
        .style(Style::new().fg(fg_color).bold())
        .alignment(Alignment::Center);
    frame.render_widget(p, size);
}
