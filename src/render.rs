//! Draw the app into a ratatui frame.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::source::ProcStatus;
use crate::tank::fish::{CreatureKind, Facing, Fish, FishState};
use crate::tank::mapping;
use crate::theme::Theme;

const WATER_TOP: Color = Color::Rgb(8, 24, 48);
const WATER_BOTTOM: Color = Color::Rgb(4, 12, 30);
const SAND: Color = Color::Rgb(86, 70, 46);
const SAND_ROCK: Color = Color::Rgb(120, 100, 70);
const SURFACE: Color = Color::Rgb(120, 190, 230);
const BUBBLE: Color = Color::Rgb(150, 205, 235);
const FOOD: Color = Color::Rgb(214, 184, 122);
const SEAWEED: Color = Color::Rgb(46, 140, 90);
const SELECT_BG: Color = Color::Rgb(40, 74, 116);
const PANEL_BG: Color = Color::Rgb(10, 26, 48);
const PANEL_FG: Color = Color::Rgb(205, 224, 244);
const MUTED: Color = Color::Rgb(140, 165, 190);
const ZOMBIE: Color = Color::DarkGray;

/// Minimum terminal size we are willing to draw a tank in.
const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 8;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
        return;
    }

    let time = app.tank.time;
    let ascii = app.config.ascii;
    let theme = &app.config.theme;

    // Day/night: as the machine gets busier the water gets darker.
    let night = night_fraction(app.load);
    let top = dim(WATER_TOP, night);
    let bottom = dim(WATER_BOTTOM, night);
    let buf = frame.buffer_mut();

    // Water background, a touch darker towards the sand.
    for y in area.y..area.bottom() {
        let t = if area.height > 1 {
            f32::from(y - area.y) / f32::from(area.height - 1)
        } else {
            0.0
        };
        let style = Style::default().bg(lerp(top, bottom, t));
        buf.set_style(Rect::new(area.x, y, area.width, 1), style);
    }

    draw_surface(buf, area, time);
    draw_sand(buf, area, ascii);
    draw_seaweed(buf, area, app, time);
    draw_bubbles(buf, area, app, ascii);
    draw_food(buf, area, app, ascii);

    for egg in &app.tank.eggs {
        let wobble = (egg.phase).sin();
        let x = egg.x.round() as i32 + wobble.round() as i32;
        let y = (area.height as i32 - 2).max(1);
        put(
            buf,
            area,
            x,
            y,
            if ascii { 'o' } else { '°' },
            Style::default().fg(mapping::color_for_name(&egg.info.name, &theme.palette)),
        );
    }

    // Reused across fish so rendering does not allocate per fish per frame.
    let mut sprites_buf: Vec<char> = Vec::with_capacity(8);
    for fish in &app.tank.fish {
        draw_fish(
            buf,
            area,
            fish,
            app.selected == Some(fish.pid),
            ascii,
            theme,
            &mut sprites_buf,
        );
    }

    if !app.config.screensaver {
        if app.show_labels {
            for fish in &app.tank.fish {
                if fish.state != FishState::Exiting {
                    draw_label(buf, area, fish, &theme.palette);
                }
            }
        }
        if let Some(fish) = app.selected_fish() {
            draw_info(buf, area, fish);
        }
        if let Some(status) = &app.status {
            draw_centered(buf, area, status, Style::default().fg(PANEL_FG));
        }
        if app.paused {
            let text = "⏸ paused";
            let x = area.right() as i32 - text.chars().count() as i32 - 2;
            draw_text(
                buf,
                area,
                x,
                area.y as i32,
                text,
                Style::default().fg(PANEL_FG),
            );
        }
    }
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let height = 3.min(area.height);
    let y = area.y + area.height.saturating_sub(height) / 2;
    let rect = Rect::new(area.x, y, area.width, height);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "procquarium",
                Style::default().fg(PANEL_FG).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled("make me bigger", Style::default().fg(MUTED))),
        ])
        .alignment(Alignment::Center),
        rect,
    );
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

fn draw_food(buf: &mut Buffer, area: Rect, app: &App, ascii: bool) {
    let ch = if ascii { '*' } else { '•' };
    for pellet in &app.tank.food {
        put(
            buf,
            area,
            pellet.x.round() as i32,
            pellet.y.round() as i32,
            ch,
            Style::default().fg(FOOD),
        );
    }
}

fn draw_fish(
    buf: &mut Buffer,
    area: Rect,
    fish: &Fish,
    selected: bool,
    ascii: bool,
    theme: &Theme,
    sprite_buf: &mut Vec<char>,
) {
    let zombie = fish.is_zombie();
    let base = if zombie {
        ZOMBIE
    } else {
        mapping::color_for_name(&fish.info.name, &theme.palette)
    };
    let mut style = Style::default().fg(base);
    if zombie {
        style = style.add_modifier(Modifier::DIM);
    }
    if fish.state == FishState::Exiting && fish.death > 0.0 {
        style = style.add_modifier(Modifier::DIM);
    }
    if fish.fed > 0.0 {
        style = style.add_modifier(Modifier::BOLD);
    }
    if selected {
        style = style.bg(SELECT_BG).add_modifier(Modifier::BOLD);
    }

    let facing_left = fish.facing == Facing::Left;
    let size = fish.size.round().clamp(0.0, 3.0) as u8;
    match fish.kind {
        CreatureKind::Crab => theme.sprites.crab_into(sprite_buf, ascii, zombie),
        CreatureKind::Jellyfish => theme.sprites.jellyfish_into(sprite_buf, ascii, zombie),
        CreatureKind::Fish if fish.state == FishState::Exiting => {
            theme
                .sprites
                .dead_fish_into(sprite_buf, size, facing_left, ascii);
        }
        CreatureKind::Fish => theme
            .sprites
            .fish_into(sprite_buf, size, facing_left, ascii, zombie),
    }

    let len = sprite_buf.len() as i32;
    let ox = fish.pos.0.round() as i32 - len / 2;
    let oy = fish.pos.1.round() as i32;
    for (i, ch) in sprite_buf.iter().enumerate() {
        put(buf, area, ox + i as i32, oy, *ch, style);
    }
}

fn draw_label(buf: &mut Buffer, area: Rect, fish: &Fish, palette: &[Color]) {
    let label: String = fish.info.name.chars().take(10).collect();
    let len = label.chars().count() as i32;
    let x = (fish.pos.0.round() as i32 - len / 2).max(i32::from(area.x));
    let y = fish.pos.1.round() as i32 + 1;
    if y < i32::from(area.bottom() - 1) {
        let style = Style::default().fg(mapping::color_for_name(&fish.info.name, palette));
        draw_text(buf, area, x, y, &label, style);
    }
}

fn draw_info(buf: &mut Buffer, area: Rect, fish: &Fish) {
    let width = 30.min(area.width.saturating_sub(2));
    let height = 7;
    if area.width < width + 2 || area.height < height + 2 {
        return;
    }
    let rect = Rect::new(
        area.right() - width - 1,
        area.bottom() - height - 1,
        width,
        height,
    );
    let info = &fish.info;
    let parent = info
        .parent
        .map(|p| p.to_string())
        .unwrap_or_else(|| "-".to_string());
    // Make the panel opaque: clear whatever decor or fish was underneath.
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            buf[(x, y)]
                .set_char(' ')
                .set_style(Style::default().bg(PANEL_BG));
        }
    }
    let status = if fish.fed > 0.0 {
        format!("{} · fed", status_str(info.status))
    } else {
        status_str(info.status).to_string()
    };
    draw_panel(
        buf,
        rect,
        &[
            (info.name.clone(), PANEL_FG, true),
            (format!("pid {:<7} ppid {}", info.pid, parent), MUTED, false),
            (format!("cpu {:.1}%", info.cpu), MUTED, false),
            (format!("mem {}", human_bytes(info.memory)), MUTED, false),
            (status, MUTED, false),
        ],
    );
}

/// Draw the info panel border and rows by hand so everything stays on the buffer.
fn draw_panel(buf: &mut Buffer, rect: Rect, rows: &[(String, Color, bool)]) {
    let border = Style::default().fg(MUTED).bg(PANEL_BG);
    for x in rect.x..rect.right() {
        put(buf, rect, i32::from(x), i32::from(rect.y), '─', border);
        put(
            buf,
            rect,
            i32::from(x),
            i32::from(rect.bottom() - 1),
            '─',
            border,
        );
    }
    for y in rect.y..rect.bottom() {
        put(buf, rect, i32::from(rect.x), i32::from(y), '│', border);
        put(
            buf,
            rect,
            i32::from(rect.right() - 1),
            i32::from(y),
            '│',
            border,
        );
    }
    let title = " details ";
    for (i, ch) in title.chars().enumerate() {
        put(
            buf,
            rect,
            i32::from(rect.x) + 1 + i as i32,
            i32::from(rect.y),
            ch,
            Style::default().fg(PANEL_FG).bg(PANEL_BG),
        );
    }
    for (row, (text, color, bold)) in rows.iter().enumerate() {
        let mut style = Style::default().fg(*color).bg(PANEL_BG);
        if *bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        draw_text(
            buf,
            rect,
            i32::from(rect.x) + 2,
            i32::from(rect.y) + 1 + row as i32,
            &text
                .chars()
                .take(rect.width as usize - 3)
                .collect::<String>(),
            style,
        );
    }
}

fn status_str(status: ProcStatus) -> &'static str {
    match status {
        ProcStatus::Running => "running",
        ProcStatus::Sleeping => "sleeping",
        ProcStatus::Zombie => "zombie",
        ProcStatus::Other => "other",
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

fn draw_centered(buf: &mut Buffer, area: Rect, text: &str, style: Style) {
    let x = i32::from(area.x) + (i32::from(area.width) - text.chars().count() as i32) / 2;
    let y = i32::from(area.y) + i32::from(area.height) / 2;
    draw_text(buf, area, x, y, text, style);
}

/// Write `text` starting at `(x, y)`, clipped to `area`.
fn draw_text(buf: &mut Buffer, area: Rect, x: i32, y: i32, text: &str, style: Style) {
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

/// How dark the water should be for a given load. Capped so a busy machine
/// looks like dusk, not a black screen.
pub fn night_fraction(load: f32) -> f32 {
    load.clamp(0.0, 1.0) * 0.65
}

/// Scale a colour towards black by `amount` (0.0 = unchanged, 1.0 = black).
pub fn dim(color: Color, amount: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let k = (1.0 - amount).clamp(0.0, 1.0);
            Color::Rgb(
                (f32::from(r) * k).round() as u8,
                (f32::from(g) * k).round() as u8,
                (f32::from(b) * k).round() as u8,
            )
        }
        other => other,
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
        let config = Config::new(
            1.0,
            60,
            None,
            None,
            false,
            false,
            false,
            false,
            Some(7),
            false,
        )
        .expect("valid");
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
        // Let the eggs hatch and the fish settle into place.
        for _ in 0..200 {
            app.update(0.03);
        }
        app.show_labels = true;
        app.select_next(false);
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
    fn tiny_terminal_shows_a_message() {
        let app = test_app(12, 4);
        let backend = TestBackend::new(12, 4);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| draw(frame, &app)).expect("draw");
        let rendered = terminal.backend().to_string();
        assert!(rendered.contains("procquarium"));
        assert!(rendered.contains("make me"));
    }

    #[test]
    fn human_bytes_reads_nicely() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
        assert_eq!(human_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    #[test]
    fn night_fraction_is_bounded_and_monotonic() {
        assert_eq!(night_fraction(0.0), 0.0);
        assert!(night_fraction(1.0) <= 0.65);
        assert!(night_fraction(0.2) < night_fraction(0.8));
        assert_eq!(night_fraction(5.0), night_fraction(1.0));
    }

    #[test]
    fn dim_scales_towards_black() {
        let c = Color::Rgb(100, 200, 50);
        assert_eq!(dim(c, 0.0), c);
        assert_eq!(dim(c, 0.5), Color::Rgb(50, 100, 25));
        assert_eq!(dim(c, 1.0), Color::Rgb(0, 0, 0));
    }
}
