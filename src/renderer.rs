use crossterm::{
    cursor,
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub color: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            color: Color::Reset,
        }
    }
}

pub struct Renderer {
    stdout: io::Stdout,
    pub width: u16,
    pub height: u16,
    buffer: Vec<Cell>,
    last_buffer: Vec<Cell>,
    kitty_enabled: bool,
}

impl Renderer {
    pub fn new() -> io::Result<Self> {
        let (width, height) = terminal::size()?;
        let stdout = io::stdout();
        // Make stdout non-blocking so a stalled reader (e.g. the terminal
        // program stopped consuming output, or a tiny pty buffer filled up)
        // can NEVER freeze the game loop. When a write would block we drop the
        // frame's output instead — gameplay keeps running.
        let fd = stdout.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        let size = (width as usize) * (height as usize);
        Ok(Self {
            stdout,
            width,
            height,
            buffer: vec![Cell::default(); size],
            last_buffer: vec![Cell::default(); size],
            kitty_enabled: false,
        })
    }

    pub fn init(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(self.stdout, EnterAlternateScreen, cursor::Hide)?;
        Ok(())
    }

    pub fn cleanup(&mut self) -> io::Result<()> {
        self.disable_kitty();
        let _ = execute!(self.stdout, LeaveAlternateScreen, cursor::Show, ResetColor);
        let _ = terminal::disable_raw_mode();
        Ok(())
    }

    /// Ask the terminal to report every key through kitty CSI-u sequences.
    /// `REPORT_ALL_KEYS_AS_ESCAPE_CODES` is required for repeat/release events
    /// on plain-text keys such as J; `REPORT_EVENT_TYPES` alone only works
    /// reliably for special keys such as arrows.
    pub fn enable_kitty(&mut self, supported: bool) -> bool {
        if supported {
            self.kitty_enabled = execute!(
                self.stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                )
            )
            .is_ok();
        }
        self.kitty_enabled
    }

    pub fn disable_kitty(&mut self) {
        if self.kitty_enabled {
            let _ = execute!(self.stdout, PopKeyboardEnhancementFlags);
            self.kitty_enabled = false;
        }
    }

    pub fn clear(&mut self) {
        self.buffer.fill(Cell::default());
    }

    pub fn put_char(&mut self, x: u16, y: u16, ch: char, color: Color) {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            if idx < self.buffer.len() {
                self.buffer[idx] = Cell { ch, color };
            }
        }
    }

    pub fn put_str(&mut self, x: u16, y: u16, text: &str, color: Color) {
        for (i, ch) in text.chars().enumerate() {
            let cx = x + i as u16;
            if cx < self.width {
                self.put_char(cx, y, ch, color);
            }
        }
    }

    #[allow(dead_code)]
    pub fn put_sprite(&mut self, x: u16, y: u16, lines: &[&str], color: Color) {
        for (row, line) in lines.iter().enumerate() {
            let cy = y + row as u16;
            if cy < self.height {
                self.put_str(x, cy, line, color);
            }
        }
    }

    /// Flush changed cells to the terminal. Writes are non-blocking; if the
    /// reader can't keep up we skip this frame's output rather than block the
    /// game loop. `WouldBlock` is therefore swallowed on purpose.
    pub fn flush(&mut self) -> io::Result<()> {
        let mut current_color = Color::Reset;
        let mut last_pos: Option<(u16, u16)> = None;

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y as usize * self.width as usize + x as usize;
                let cell = self.buffer[idx];
                let last = self.last_buffer[idx];

                if cell != last {
                    let expected = last_pos.map(|(lx, ly)| (lx + 1, ly));
                    if expected != Some((x, y)) {
                        ignore_blocked(queue!(self.stdout, cursor::MoveTo(x, y)))?;
                    }
                    if cell.color != current_color {
                        ignore_blocked(queue!(self.stdout, SetForegroundColor(cell.color)))?;
                        current_color = cell.color;
                    }
                    ignore_blocked(queue!(self.stdout, Print(cell.ch)))?;
                    last_pos = Some((x, y));
                }
            }
        }

        if current_color != Color::Reset {
            ignore_blocked(queue!(self.stdout, ResetColor))?;
        }
        ignore_blocked(self.stdout.flush())?;
        self.last_buffer.copy_from_slice(&self.buffer);
        Ok(())
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let size = (width as usize) * (height as usize);
        self.buffer = vec![Cell::default(); size];
        self.last_buffer = vec![Cell::default(); size];
    }
}

/// Run a crossterm `queue!`/`flush` that writes to a non-blocking stdout.
/// If the write would block (reader is behind), drop the frame rather than
/// freezing the game loop. Other errors still propagate.
fn ignore_blocked(r: io::Result<()>) -> io::Result<()> {
    match r {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(e) => Err(e),
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
