//! Draw the app into a ratatui frame.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::App;
use crate::tank::fish::{Facing, Fish};
use crate::tank::{mapping, sprites};

const WATER: Color = Color::Rgb(6, 18, 38);
const MUTED: Color = Color::Rgb(140, 165, 190);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let ascii = app.config.ascii;
    let buf = frame.buffer_mut();

    buf.set_style(area, Style::default().bg(WATER));

    for fish in &app.tank.fish {
        draw_fish(buf, area, fish, ascii);
    }

    if !app.ready
        && let Some(status) = &app.status
    {
        draw_centered(buf, area, status, Style::default().fg(MUTED));
    }
}

fn draw_fish(buf: &mut Buffer, area: Rect, fish: &Fish, ascii: bool) {
    let style = Style::default().fg(mapping::color_for_name(&fish.info.name));
    let sprite = sprites::sprite(
        fish.size.round().clamp(0.0, 3.0) as u8,
        fish.facing == Facing::Left,
        ascii,
        false,
    );
    let len = sprite.len() as i32;
    let ox = fish.pos.0.round() as i32 - len / 2;
    let oy = fish.pos.1.round() as i32;
    for (i, ch) in sprite.iter().enumerate() {
        put(buf, area, ox + i as i32, oy, *ch, style);
    }
}

fn draw_centered(buf: &mut Buffer, area: Rect, text: &str, style: Style) {
    let x = i32::from(area.x) + (i32::from(area.width) - text.chars().count() as i32) / 2;
    let y = i32::from(area.y) + i32::from(area.height) / 2;
    for (i, ch) in text.chars().enumerate() {
        put(buf, area, x + i as i32, y, ch, style);
    }
}

/// Write one cell if it is inside `area`.
fn put(buf: &mut Buffer, area: Rect, x: i32, y: i32, ch: char, style: Style) {
    if x < i32::from(area.x)
        || y < i32::from(area.y)
        || x >= i32::from(area.right())
        || y >= i32::from(area.bottom())
    {
        return;
    }
    let cell = &mut buf[(x as u16, y as u16)];
    cell.set_char(ch).set_style(style);
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::source::ProcessSource;
    use crate::source::fake::{FakeSource, proc};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn test_app(width: u16, height: u16) -> App {
        let config = Config::new(1.0, 60, None, None, false, false).expect("valid");
        let mut source = FakeSource::constant(vec![
            proc(1, "firefox")
                .with_memory(2 * 1024 * 1024 * 1024)
                .with_cpu(35.0),
            proc(2, "cargo")
                .with_memory(200 * 1024 * 1024)
                .with_cpu(80.0),
            proc(3, "zsh").with_memory(4 * 1024 * 1024),
            proc(4, "kworker").kernel_thread(),
        ]);
        let mut app = App::new(config, width, height, 7);
        app.apply_snapshot(source.snapshot().expect("sample"));
        app
    }

    #[test]
    fn tank_snapshot_is_stable() {
        let app = test_app(60, 20);
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| draw(frame, &app)).expect("draw");
        insta::assert_snapshot!(terminal.backend().to_string());
    }

    #[test]
    fn human_bytes_reads_nicely() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
        assert_eq!(human_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }
}
