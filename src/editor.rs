use crate::model::*;

use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
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
    memo: Vec<HashMap<&'static str, usize>>,
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
            memo: (0..n).map(|_| HashMap::new()).collect(),
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
        if self.row >= nlines {
            self.row = nlines - 1;
        }
        let len = self.line().len();
        if self.col > len {
            self.col = len;
        }
    }

    fn save_memo(&mut self) {
        let m = &mut self.memo[self.si];
        m.insert("row", self.row);
        m.insert("col", self.col);
        m.insert("scroll", self.scroll);
    }

    fn restore_memo(&mut self) {
        let m = &self.memo[self.si];
        self.row = m.get("row").copied().unwrap_or(0);
        self.col = m.get("col").copied().unwrap_or(0);
        self.scroll = m.get("scroll").copied().unwrap_or(0);
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

        execute!(stdout, Clear(ClearType::All))?;

        // ── Tab bar (row 0) ──
        execute!(stdout, MoveTo(0, 0))?;
        for (i, _) in self.steps.iter().enumerate() {
            if i == self.si {
                execute!(
                    stdout,
                    SetBackgroundColor(Color::Cyan),
                    SetForegroundColor(Color::Black),
                    SetAttribute(Attribute::Bold)
                )?;
            } else {
                execute!(
                    stdout,
                    SetBackgroundColor(Color::DarkGrey),
                    SetForegroundColor(Color::White)
                )?;
            }
            let label = format!(" {} ", i + 1);
            write!(stdout, "{}", label)?;
            execute!(stdout, ResetColor)?;
            write!(stdout, " ")?;
        }
        // right-align filename
        let tabs_width = self.steps.len() * 4; // " N " + " " per tab
        let fname = &self.filename;
        if tabs_width + fname.len() < tw {
            let pad = tw - tabs_width - fname.len();
            write!(stdout, "{:>width$}", fname, width = pad + fname.len())?;
        }

        // ── Content area ──
        let step = self.step();
        let mut cursor_screen_row: u16 = 1;
        let mut cursor_screen_col: u16 = 0;

        for vrow in 0..content_h {
            let abs_row = self.scroll + vrow;
            let screen_y = (vrow + 1) as u16; // +1 for tab bar
            execute!(stdout, MoveTo(0, screen_y))?;

            if abs_row >= step.len() {
                // empty line beyond content
                execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
                write!(stdout, "~")?;
                execute!(stdout, ResetColor)?;
                continue;
            }

            let line = &step[abs_row];
            let is_first_line = abs_row == 0;

            // Set color: first line green, rest white
            if is_first_line {
                execute!(stdout, SetForegroundColor(Color::Green))?;
            } else {
                execute!(stdout, SetForegroundColor(Color::White))?;
            }

            // Render line content (no cloning, just iterate chars)
            let display: String = line.chars().take(tw).collect();
            write!(stdout, "{}", display)?;
            execute!(stdout, ResetColor)?;

            // Track cursor position
            if abs_row == self.row {
                cursor_screen_row = screen_y;
                cursor_screen_col = self.col.min(tw.saturating_sub(1)) as u16;
            }
        }

        // ── Status bar (last row) ──
        let status_y = (th - 1) as u16;
        execute!(stdout, MoveTo(0, status_y))?;

        let (bg, fg) = match &self.msg {
            Some((_, MsgKind::Error)) => (Color::Red, Color::White),
            Some((_, MsgKind::Warn)) => (Color::Yellow, Color::Black),
            _ => (Color::Green, Color::Black),
        };
        execute!(
            stdout,
            SetBackgroundColor(bg),
            SetForegroundColor(fg)
        )?;

        let dirty_ch = if self.dirty { '*' } else { ' ' };
        let nsteps = self.steps.len();
        let ln = self.row + 1;

        let left = if let Some((ref text, _)) = self.msg {
            format!(
                "{}Step {}/{} Ln {} | {}",
                dirty_ch,
                self.si + 1,
                nsteps,
                ln,
                text
            )
        } else {
            format!(
                "{}Step {}/{} Ln {} | ^N/^P:Step ^L:Clear ^S:Save ^Q:Quit",
                dirty_ch,
                self.si + 1,
                nsteps,
                ln,
            )
        };

        let display: String = if left.len() < tw {
            format!("{:<width$}", left, width = tw)
        } else {
            left.chars().take(tw).collect()
        };
        write!(stdout, "{}", display)?;
        execute!(stdout, ResetColor)?;

        // Position cursor
        execute!(stdout, MoveTo(cursor_screen_col, cursor_screen_row))?;

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
            // ── Ctrl combinations ──
            KeyCode::Char('c') if ctrl => {
                self.running = false;
            }
            KeyCode::Char('q') if ctrl => {
                if !self.dirty {
                    self.running = false;
                } else if self.quit_confirm {
                    self.running = false;
                } else {
                    self.quit_confirm = true;
                    self.set_msg("Unsaved! ^Q again to discard", MsgKind::Warn);
                }
            }
            KeyCode::Char('s') if ctrl => {
                self.save();
            }
            KeyCode::Char('n') if ctrl => {
                self.next_step();
            }
            KeyCode::Char('p') if ctrl => {
                self.prev_step();
            }
            KeyCode::Char('a') if ctrl => {
                self.col = 0;
            }
            KeyCode::Char('e') if ctrl => {
                self.col = self.line().len();
            }
            KeyCode::Char('l') if ctrl => {
                self.clear_step();
            }

            // ── Step switching ──
            KeyCode::PageDown => {
                self.next_step();
            }
            KeyCode::PageUp => {
                self.prev_step();
            }

            // ── Tab key inserts literal tab ──
            KeyCode::Tab => {
                self.insert_tab();
            }

            // ── Navigation ──
            KeyCode::Left => {
                if self.col > 0 {
                    self.col -= 1;
                }
            }
            KeyCode::Right => {
                if self.col < self.line().len() {
                    self.col += 1;
                }
            }
            KeyCode::Up => {
                self.move_up();
            }
            KeyCode::Down => {
                self.move_down();
            }
            KeyCode::Home => {
                self.col = 0;
            }
            KeyCode::End => {
                self.col = self.line().len();
            }

            // ── Editing ──
            KeyCode::Enter => {
                self.insert_newline();
            }
            KeyCode::Backspace => {
                self.handle_backspace();
            }
            KeyCode::Delete => {
                self.handle_delete();
            }
            KeyCode::Char(c) if !ctrl && (c as u32 >= 32 && c as u32 <= 126) => {
                self.insert_char(c);
            }

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

    fn next_step(&mut self) {
        if self.si + 1 < self.steps.len() {
            self.switch_step(self.si + 1);
        }
    }

    fn prev_step(&mut self) {
        if self.si > 0 {
            self.switch_step(self.si - 1);
        }
    }

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

    fn insert_tab(&mut self) {
        self.insert_char('\t');
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
                Event::Resize(_, _) => {} // redraw on next iteration
                _ => {}
            }

            // paste batching: drain all queued events before redrawing
            while event::poll(Duration::ZERO)? {
                match event::read()? {
                    Event::Key(key) => self.handle_key(key),
                    _ => {}
                }
                if !self.running {
                    break;
                }
            }
        }
        Ok(())
    }
}
