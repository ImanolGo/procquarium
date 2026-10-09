//! Draw the app into a ratatui frame.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::App;
use crate::tank::fish::{Facing, Fish};
use crate::tank::{mapping, sprites};

const WATER_TOP: Color = Color::Rgb(8, 24, 48);
const WATER_BOTTOM: Color = Color::Rgb(4, 12, 30);
const SAND: Color = Color::Rgb(86, 70, 46);
const SAND_ROCK: Color = Color::Rgb(120, 100, 70);
const SURFACE: Color = Color::Rgb(120, 190, 230);
const BUBBLE: Color = Color::Rgb(150, 205, 235);
const SEAWEED: Color = Color::Rgb(46, 140, 90);
const MUTED: Color = Color::Rgb(140, 165, 190);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let time = app.tank.time;
    let ascii = app.config.ascii;
    let buf = frame.buffer_mut();

    // Water background, a touch darker towards the sand.
    for y in area.y..area.bottom() {
        let t = if area.height > 1 {
            f32::from(y - area.y) / f32::from(area.height - 1)
        } else {
            0.0
        };
        buf.set_style(
            Rect::new(area.x, y, area.width, 1),
            Style::default().bg(lerp(WATER_TOP, WATER_BOTTOM, t)),
        );
    }

    draw_surface(buf, area, time);
    draw_sand(buf, area, ascii);
    draw_seaweed(buf, area, app, time);
    draw_bubbles(buf, area, app, ascii);

    for fish in &app.tank.fish {
        draw_fish(buf, area, fish, ascii);
    }

    if !app.ready
        && let Some(status) = &app.status
    {
        draw_centered(buf, area, status, Style::default().fg(MUTED));
    }
}

fn draw_surface(buf: &mut Buffer, area: Rect, time: f32) {
    let y = area.y;
    let mut x = area.x as i32;
    while (x as u16) < area.right() {
        let wave = (x as f32 * 0.35 + time * 1.5).sin();
        let (ch, width) = if wave > 0.1 {
            ('~', 1)
        } else if wave < -0.6 {
            ('~', 2)
        } else {
            (' ', 1)
        };
        put(buf, area, x, i32::from(y), ch, Style::default().fg(SURFACE));
        x += width;
    }
}

fn draw_sand(buf: &mut Buffer, area: Rect, ascii: bool) {
    let y = i32::from(area.bottom() - 1);
    for x in area.x..area.right() {
        let (ch, color) = if (!ascii && x % 11 == 4) || (ascii && x % 11 == 5) {
            ('^', SAND_ROCK)
        } else if x % 3 == 0 {
            ('.', SAND)
        } else {
            (if ascii { '.' } else { '·' }, SAND)
        };
        put(buf, area, i32::from(x), y, ch, Style::default().fg(color));
    }
}

fn draw_seaweed(buf: &mut Buffer, area: Rect, app: &App, time: f32) {
    let sand_y = i32::from(area.bottom() - 2);
    for strand in &app.tank.decor.strands {
        for seg in 0..strand.segments {
            let sway = (time * strand.speed + strand.phase + seg as f32 * 0.6).sin()
                * (0.4 + seg as f32 * 0.3);
            let x = strand.x + sway;
            let y = sand_y - seg as i32;
            let ch = if seg + 1 == strand.segments {
                '~'
            } else if sway > 0.0 {
                ')'
            } else {
                '('
            };
            put(
                buf,
                area,
                x.round() as i32,
                y,
                ch,
                Style::default().fg(SEAWEED),
            );
        }
    }
}

fn draw_bubbles(buf: &mut Buffer, area: Rect, app: &App, ascii: bool) {
    for b in &app.tank.decor.bubbles {
        let ch = match (ascii, b.big) {
            (_, true) => 'o',
            (true, false) => '.',
            (false, false) => '°',
        };
        put(
            buf,
            area,
            b.x.round() as i32,
            b.y.round() as i32,
            ch,
            Style::default().fg(BUBBLE),
        );
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

fn lerp(a: Color, b: Color, t: f32) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let t = t.clamp(0.0, 1.0);
            let mix =
                |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
            Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
        }
        _ => a,
    }
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
        let config = Config::new(1.0, 60, None, None, false, false, Some(7)).expect("valid");
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
        for _ in 0..200 {
            app.update(0.03);
        }
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
