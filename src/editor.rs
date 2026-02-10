use crate::model::*;

use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor::{self, MoveTo},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
    },
};

#[derive(Clone, Copy)]
pub enum MsgKind {
    Ok,
    Warn,
    Error,
}

pub struct Editor {
    pub filename: String,
    path: std::path::PathBuf,
    pub steps: Vec<Step>,
    memo: Vec<(usize, usize, usize)>,
    si: usize,
    row: usize,
    col: usize,
    scroll: usize,
    dirty: bool,
    msg: Option<(String, MsgKind)>,
    running: bool,
    quit_confirm: bool,
}

impl Editor {
    pub fn new(filename: String, steps: Vec<Step>) -> Self {
        let n = steps.len();
        let path = std::path::PathBuf::from(&filename);
        Self {
            filename,
            path,
            steps,
            memo: vec![(0, 0, 0); n],
            si: 0,
            row: 0,
            col: 0,
            scroll: 0,
            dirty: false,
            msg: None,
            running: true,
            quit_confirm: false,
        }
    }

    // ── helpers ───────────────────────────────────────────────

    fn step(&self) -> &Step {
        &self.steps[self.si]
    }

    fn step_mut(&mut self) -> &mut Step {
        &mut self.steps[self.si]
    }

    fn line(&self) -> &String {
        &self.steps[self.si][self.row]
    }

    fn line_mut(&mut self) -> &mut String {
        &mut self.steps[self.si][self.row]
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.quit_confirm = false;
    }

    fn set_msg(&mut self, text: impl Into<String>, kind: MsgKind) {
        self.msg = Some((text.into(), kind));
    }

    fn clamp_cursor(&mut self) {
        let nlines = self.step().len();
        if nlines == 0 {
            self.row = 0;
            self.col = 0;
            return;
        }
        self.row = self.row.min(nlines - 1);
        self.col = self.col.min(self.line().len());
    }

    fn save_memo(&mut self) {
        self.memo[self.si] = (self.row, self.col, self.scroll);
    }

    fn restore_memo(&mut self) {
        let (row, col, scroll) = self.memo[self.si];
        self.row = row;
        self.col = col;
        self.scroll = scroll;
        self.clamp_cursor();
    }

    fn ensure_cursor_visible(&mut self, content_h: usize) {
        if self.row < self.scroll {
            self.scroll = self.row;
        } else if self.row >= self.scroll + content_h {
            self.scroll = self.row + 1 - content_h;
        }
    }

    // ── drawing ──────────────────────────────────────────────

    fn draw(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
        let (tw, th) = terminal::size()?;
        let tw = tw as usize;
        let th = th as usize;
        if th < 3 {
            return Ok(());
        }
        let content_h = th - 2; // 1 tab bar + 1 status bar

        self.ensure_cursor_visible(content_h);

        queue!(stdout, cursor::Hide, Clear(ClearType::All))?;

        // ── Tab bar (row 0) ──
        queue!(stdout, MoveTo(0, 0))?;
        for i in 0..self.steps.len() {
            if i == self.si {
                queue!(stdout, SetBackgroundColor(Color::Cyan), SetForegroundColor(Color::Black), SetAttribute(Attribute::Bold))?;
            } else {
                queue!(stdout, SetBackgroundColor(Color::DarkGrey), SetForegroundColor(Color::White))?;
            }
            write!(stdout, " {} ", i + 1)?;
            queue!(stdout, ResetColor)?;
            write!(stdout, " ")?;
        }
        let tabs_width = self.steps.len() * 4;
        if tabs_width + self.filename.len() < tw {
            write!(stdout, "{:>width$}", self.filename, width = tw - tabs_width)?;
        }

        // ── Content area ──
        let step = self.step();
        let mut cursor_screen_row: u16 = 1;
        let mut cursor_screen_col: u16 = 0;

        for vrow in 0..content_h {
            let abs_row = self.scroll + vrow;
            let screen_y = (vrow + 1) as u16;
            queue!(stdout, MoveTo(0, screen_y))?;

            if abs_row >= step.len() {
                queue!(stdout, SetForegroundColor(Color::DarkGrey))?;
                write!(stdout, "~")?;
                queue!(stdout, ResetColor)?;
                continue;
            }

            let line = &step[abs_row];
            let color = if abs_row == 0 { Color::Green } else { Color::White };
            queue!(stdout, SetForegroundColor(color))?;

            let display: String = line.chars().take(tw).collect();
            write!(stdout, "{}", display)?;
            queue!(stdout, ResetColor)?;

            if abs_row == self.row {
                cursor_screen_row = screen_y;
                cursor_screen_col = self.col.min(tw.saturating_sub(1)) as u16;
            }
        }

        // ── Status bar (last row) ──
        let status_y = (th - 1) as u16;
        queue!(stdout, MoveTo(0, status_y))?;

        let (bg, fg) = match &self.msg {
            Some((_, MsgKind::Error)) => (Color::Red, Color::White),
            Some((_, MsgKind::Warn)) => (Color::Yellow, Color::Black),
            _ => (Color::Green, Color::Black),
        };
        queue!(stdout, SetBackgroundColor(bg), SetForegroundColor(fg))?;

        let dirty_ch = if self.dirty { '*' } else { ' ' };
        let nsteps = self.steps.len();
        let ln = self.row + 1;

        let info = format!("{}Step {}/{} Ln {}", dirty_ch, self.si + 1, nsteps, ln);
        let left = match &self.msg {
            Some((text, _)) => format!("{} | {}", info, text),
            None => format!("{} | ^N/^P:Step ^L:Clear ^S:Save ^Q:Quit", info),
        };

        let display: String = if left.len() < tw {
            format!("{:<width$}", left, width = tw)
        } else {
            left.chars().take(tw).collect()
        };
        write!(stdout, "{}", display)?;
        queue!(stdout, ResetColor)?;

        queue!(stdout, MoveTo(cursor_screen_col, cursor_screen_row), cursor::Show)?;

        stdout.flush()?;
        Ok(())
    }

    // ── input handling ───────────────────────────────────────

    fn handle_key(&mut self, key: KeyEvent) {
        let mods = key.modifiers;
        let ctrl = mods.contains(KeyModifiers::CONTROL);

        // Clear transient message on any key (except repeat quit)
        if key.code != KeyCode::Char('q') || !ctrl {
            self.msg = None;
        }

        // If quit_confirm is set and we get anything other than Ctrl+Q, cancel it
        if self.quit_confirm && !(ctrl && key.code == KeyCode::Char('q')) {
            self.quit_confirm = false;
        }

        match key.code {
            KeyCode::Char('c') if ctrl => self.running = false,
            KeyCode::Char('q') if ctrl => {
                if !self.dirty || self.quit_confirm {
                    self.running = false;
                } else {
                    self.quit_confirm = true;
                    self.set_msg("Unsaved! ^Q again to discard", MsgKind::Warn);
                }
            }
            KeyCode::Char('s') if ctrl => self.save(),
            KeyCode::Char('n') if ctrl => self.next_step(),
            KeyCode::Char('p') if ctrl => self.prev_step(),
            KeyCode::Char('a') if ctrl => self.col = 0,
            KeyCode::Char('e') if ctrl => self.col = self.line().len(),
            KeyCode::Char('l') if ctrl => self.clear_step(),
            KeyCode::PageDown => self.next_step(),
            KeyCode::PageUp => self.prev_step(),
            KeyCode::Tab => self.insert_char('\t'),
            KeyCode::Left => { if self.col > 0 { self.col -= 1; } }
            KeyCode::Right => { if self.col < self.line().len() { self.col += 1; } }
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Home => self.col = 0,
            KeyCode::End => self.col = self.line().len(),
            KeyCode::Enter => self.insert_newline(),
            KeyCode::Backspace => self.handle_backspace(),
            KeyCode::Delete => self.handle_delete(),
            KeyCode::Char(c) if !ctrl && !c.is_control() => self.insert_char(c),
            _ => {}
        }
    }

    // ── step switching ───────────────────────────────────────

    fn switch_step(&mut self, new_si: usize) {
        if new_si == self.si || new_si >= self.steps.len() {
            return;
        }
        self.save_memo();
        self.si = new_si;
        self.restore_memo();
    }

    fn next_step(&mut self) { self.switch_step(self.si + 1); }

    fn prev_step(&mut self) { self.switch_step(self.si.saturating_sub(1)); }

    // ── cursor movement ──────────────────────────────────────

    fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.line().len());
        }
    }

    fn move_down(&mut self) {
        if self.row + 1 < self.step().len() {
            self.row += 1;
            self.col = self.col.min(self.line().len());
        }
    }

    // ── text editing ─────────────────────────────────────────

    fn insert_char(&mut self, c: char) {
        let col = self.col;
        self.line_mut().insert(col, c);
        self.col += 1;
        self.mark_dirty();
    }

    fn insert_newline(&mut self) {
        let col = self.col;
        let rest = self.line_mut().split_off(col);
        let row = self.row;
        self.step_mut().insert(row + 1, rest);
        self.row += 1;
        self.col = 0;
        self.mark_dirty();
    }

    fn handle_backspace(&mut self) {
        if self.col > 0 {
            let col = self.col - 1;
            self.line_mut().remove(col);
            self.col = col;
            self.mark_dirty();
        } else if self.row > 0 {
            // join with previous line
            let row = self.row;
            let current_line = self.step_mut().remove(row);
            self.row -= 1;
            self.col = self.line().len();
            self.line_mut().push_str(&current_line);
            self.mark_dirty();
        }
    }

    fn handle_delete(&mut self) {
        let len = self.line().len();
        if self.col < len {
            let col = self.col;
            self.line_mut().remove(col);
            self.mark_dirty();
        } else if self.row + 1 < self.step().len() {
            // join with next line
            let row = self.row;
            let next_line = self.step_mut().remove(row + 1);
            self.line_mut().push_str(&next_line);
            self.mark_dirty();
        }
    }

    fn clear_step(&mut self) {
        self.steps[self.si] = crate::model::new_step();
        self.row = 0;
        self.col = 0;
        self.scroll = 0;
        self.mark_dirty();
        self.set_msg("Step cleared", MsgKind::Ok);
    }

    // ── save ─────────────────────────────────────────────────

    fn save(&mut self) {
        match crate::file_io::save_file(&self.path, &self.steps) {
            Ok(()) => {
                self.dirty = false;
                self.set_msg("Saved", MsgKind::Ok);
            }
            Err(e) => {
                self.set_msg(format!("Save error: {}", e), MsgKind::Error);
            }
        }
    }

    // ── main loop ────────────────────────────────────────────

    pub fn run(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();

        terminal::enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;

        let result = self.event_loop(&mut stdout);

        execute!(stdout, LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;

        result
    }

    fn event_loop(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
        while self.running {
            self.draw(stdout)?;

            match event::read()? {
                Event::Key(key) => self.handle_key(key),
                Event::Resize(_, _) => continue,
                _ => {}
            }

            while event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
                if !self.running {
                    break;
                }
            }
        }
        Ok(())
    }
}
