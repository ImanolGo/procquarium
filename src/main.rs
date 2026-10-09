//! procquarium: your running processes, as fish.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    loop {
        terminal.draw(draw)?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(());
        }
    }
}

fn draw(frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(
        Block::bordered()
            .style(Style::default().bg(Color::Blue))
            .title("procquarium"),
        area,
    );
}
