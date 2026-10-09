//! Draw the app into a ratatui frame.

use std::collections::HashSet;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::config::ColorMode;
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
const SPARK: Color = Color::Rgb(126, 200, 160);
const ZOMBIE: Color = Color::DarkGray;

/// Minimum terminal size we are willing to draw a tank in.
const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 8;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
    } else {
        draw_tank(frame, app, area);
    }
    // One place turns the RGB colours into whatever the terminal can show.
    apply_color_mode(frame.buffer_mut(), area, app.config.colors.resolved());
}

fn draw_tank(frame: &mut Frame, app: &App, area: Rect) {
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

    // Family of the selected fish: highlight the parent and children, and run
    // a faint dotted line to the parent.
    let mut family: HashSet<u32> = HashSet::new();
    if let Some(fish) = app.selected_fish() {
        if let Some(parent) = fish.info.parent {
            family.insert(parent);
        }
        for other in &app.tank.fish {
            if other.state != FishState::Exiting && other.info.parent == Some(fish.pid) {
                family.insert(other.pid);
            }
        }
        if let Some(parent) = fish.info.parent.and_then(|pid| app.tank.fish(pid)) {
            draw_family_line(buf, area, fish.pos, parent.pos);
        }
    }

    // Reused across fish so rendering does not allocate per fish per frame.
    let mut sprites_buf: Vec<char> = Vec::with_capacity(8);
    for fish in &app.tank.fish {
        draw_fish(
            buf,
            area,
            fish,
            app.selected == Some(fish.pid),
            family.contains(&fish.pid),
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
            let sparkline = app.cpu_sparkline(fish.identity());
            draw_info(buf, area, fish, &sparkline);
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
        if let Some(query) = app.search_query() {
            let y = area.bottom() as i32 - 1;
            let style = Style::default().bg(PANEL_BG).fg(PANEL_FG);
            for x in area.x..area.right() {
                put(buf, area, i32::from(x), y, ' ', style);
            }
            draw_text(
                buf,
                area,
                i32::from(area.x) + 1,
                y,
                &format!("/{query}"),
                style,
            );
        }
        if let Some(info) = app.kill_prompt() {
            let y = area.bottom() as i32 - 1;
            let style = Style::default()
                .bg(PANEL_BG)
                .fg(PANEL_FG)
                .add_modifier(Modifier::BOLD);
            for x in area.x..area.right() {
                put(
                    buf,
                    area,
                    i32::from(x),
                    y,
                    ' ',
                    Style::default().bg(PANEL_BG),
                );
            }
            let text = format!("send SIGTERM to {} ({})? y/n", info.name, info.pid);
            draw_text(buf, area, i32::from(area.x) + 1, y, &text, style);
        }
    }
}

/// Rewrite every cell's colours for terminals that cannot show 24-bit RGB:
/// drop them for `None`, or snap each to the nearest of the 256 xterm colours.
/// `Truecolor` leaves the buffer untouched.
fn apply_color_mode(buf: &mut Buffer, area: Rect, mode: ColorMode) {
    match mode {
        ColorMode::Truecolor => {}
        ColorMode::Ansi256 => {
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    let cell = &mut buf[(x, y)];
                    cell.fg = nearest_256(cell.fg);
                    cell.bg = nearest_256(cell.bg);
                }
            }
        }
        ColorMode::None => {
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    let cell = &mut buf[(x, y)];
                    cell.set_fg(Color::Reset);
                    cell.set_bg(Color::Reset);
                    cell.modifier = Modifier::empty();
                }
            }
        }
        // `resolved()` never returns `Auto`; treat it as truecolour if it does.
        ColorMode::Auto => {}
    }
}

/// The RGB value of one of the 256 xterm colours.
fn xterm_rgb(index: u8) -> (u8, u8, u8) {
    const SYSTEM: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match index {
        0..=15 => SYSTEM[usize::from(index)],
        16..=231 => {
            let i = index - 16;
            let level = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            (level(i / 36), level((i % 36) / 6), level(i % 6))
        }
        _ => {
            let v = 8 + (index - 232) * 10;
            (v, v, v)
        }
    }
}

/// Snap an RGB colour to the nearest of the 256 xterm colours. `Reset` and
/// already-indexed colours pass through unchanged.
fn nearest_256(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let mut best = 0u8;
    let mut best_distance = u32::MAX;
    for index in 0..=255u8 {
        let (cr, cg, cb) = xterm_rgb(index);
        let dr = i32::from(r) - i32::from(cr);
        let dg = i32::from(g) - i32::from(cg);
        let db = i32::from(b) - i32::from(cb);
        let distance = (dr * dr + dg * dg + db * db) as u32;
        if distance < best_distance {
            best_distance = distance;
            best = index;
        }
    }
    Color::Indexed(best)
}

/// A barnacle for a long-running process: `·` after a day, `:` after a week.
pub fn barnacle(run_time: u64) -> Option<char> {
    const DAY: u64 = 24 * 60 * 60;
    const WEEK: u64 = 7 * DAY;
    if run_time >= WEEK {
        Some(':')
    } else if run_time >= DAY {
        Some('·')
    } else {
        None
    }
}

/// The pid of the living creature drawn under a cell, if any. Used by mouse
/// clicks; drawn later (further along the vector) means on top.
pub fn fish_at(app: &App, column: u16, row: u16) -> Option<u32> {
    let ascii = app.config.ascii;
    let (cx, cy) = (i32::from(column), i32::from(row));
    let mut buf = Vec::with_capacity(8);
    for fish in app.tank.fish.iter().rev() {
        if fish.state == FishState::Exiting {
            continue;
        }
        let half = sprite_len(&app.config.theme.sprites, fish, ascii, &mut buf) / 2;
        let ox = fish.pos.0.round() as i32;
        let oy = fish.pos.1.round() as i32;
        if (cx - ox).abs() <= half && (cy - oy).abs() <= 1 {
            return Some(fish.pid);
        }
    }
    None
}

fn sprite_len(
    sprites: &crate::tank::sprites::Sprites,
    fish: &Fish,
    ascii: bool,
    buf: &mut Vec<char>,
) -> i32 {
    match fish.kind {
        CreatureKind::Crab => sprites.crab_into(buf, ascii, false),
        CreatureKind::Jellyfish => sprites.jellyfish_into(buf, ascii, false),
        CreatureKind::Fish => {
            let size = fish.size.round().clamp(0.0, 3.0) as u8;
            if fish.state == FishState::Exiting {
                sprites.dead_fish_into(buf, size, false, ascii);
            } else {
                sprites.fish_into(buf, size, false, ascii, false);
            }
        }
    }
    buf.len() as i32
}

/// A faint dotted line between the selected fish and its parent.
fn draw_family_line(buf: &mut Buffer, area: Rect, from: (f32, f32), to: (f32, f32)) {
    let style = Style::default().fg(MUTED).add_modifier(Modifier::DIM);
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let steps = dx.hypot(dy).ceil().max(1.0) as i32;
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let x = (from.0 + dx * t).round() as i32;
        let y = (from.1 + dy * t).round() as i32;
        put(buf, area, x, y, '·', style);
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

#[allow(clippy::too_many_arguments)]
fn draw_fish(
    buf: &mut Buffer,
    area: Rect,
    fish: &Fish,
    selected: bool,
    family: bool,
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
    if fish.appear > 0.0 {
        style = style.add_modifier(Modifier::DIM);
    }
    if selected {
        style = style.bg(SELECT_BG).add_modifier(Modifier::BOLD);
    }
    if family {
        style = style.add_modifier(Modifier::UNDERLINED);
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

    if let Some(mark) = barnacle(fish.info.run_time) {
        sprite_buf.push(mark);
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

fn draw_info(buf: &mut Buffer, area: Rect, fish: &Fish, sparkline: &str) {
    let width = 30.min(area.width.saturating_sub(2));
    let height = 8;
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
            (sparkline.to_string(), SPARK, false),
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
        let config = Config::builder()
            .max_fish(60)
            .seed(Some(7))
            .build()
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
    fn selecting_a_fish_draws_a_line_to_its_parent() {
        let config = Config::builder()
            .max_fish(60)
            .seed(Some(7))
            .build()
            .expect("valid");
        let mut source =
            FakeSource::constant(vec![proc(1, "parent"), proc(2, "child").with_parent(1)]);
        let mut app = App::new(config, 60, 20, 7);
        app.apply_snapshot(source.snapshot().expect("sample"));
        for _ in 0..150 {
            app.update(0.03);
        }
        // Put them far apart and select the child.
        app.tank.fish[0].pos = (10.0, 5.0);
        app.tank.fish[1].pos = (50.0, 15.0);
        app.selected = Some(2);

        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| draw(frame, &app)).expect("draw");
        assert!(
            terminal.backend().to_string().contains('·'),
            "a dotted line should connect the child to its parent"
        );
    }

    #[test]
    fn old_fish_grow_barnacles() {
        assert_eq!(barnacle(0), None);
        assert_eq!(barnacle(24 * 60 * 60), Some('·'));
        assert_eq!(barnacle(8 * 24 * 60 * 60), Some(':'));

        let config = Config::builder()
            .max_fish(60)
            .seed(Some(7))
            .build()
            .expect("valid");
        let old = proc(1, "old").with_run_time(2 * 24 * 60 * 60);
        let mut source = FakeSource::constant(vec![old]);
        let mut app = App::new(config, 60, 20, 7);
        app.apply_snapshot(source.snapshot().expect("sample"));
        for _ in 0..150 {
            app.update(0.03);
        }
        app.tank.fish[0].pos = (30.0, 10.0);

        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| draw(frame, &app)).expect("draw");
        assert!(terminal.backend().to_string().contains('·'));
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

    #[test]
    fn xterm_rgb_knows_the_cube_and_the_grayscale_ramp() {
        assert_eq!(xterm_rgb(16), (0, 0, 0));
        assert_eq!(xterm_rgb(196), (255, 0, 0));
        assert_eq!(xterm_rgb(231), (255, 255, 255));
        assert_eq!(xterm_rgb(232), (8, 8, 8));
        assert_eq!(xterm_rgb(255), (238, 238, 238));
    }

    #[test]
    fn nearest_256_picks_the_closest_colour() {
        assert_eq!(nearest_256(Color::Rgb(95, 0, 0)), Color::Indexed(52));
        assert_eq!(nearest_256(Color::Rgb(255, 215, 0)), Color::Indexed(220));
        assert_eq!(nearest_256(Color::Rgb(8, 8, 8)), Color::Indexed(232));
        // Anything that is not RGB passes through untouched.
        assert_eq!(nearest_256(Color::Reset), Color::Reset);
        assert_eq!(nearest_256(Color::DarkGray), Color::DarkGray);
    }

    #[test]
    fn apply_color_mode_snaps_or_drops_colours() {
        let area = Rect::new(0, 0, 1, 1);

        let mut buf = Buffer::empty(area);
        buf[(0, 0)]
            .set_fg(Color::Rgb(95, 0, 0))
            .set_bg(Color::Rgb(8, 8, 8));
        apply_color_mode(&mut buf, area, ColorMode::Ansi256);
        assert_eq!(buf[(0, 0)].fg, Color::Indexed(52));
        assert_eq!(buf[(0, 0)].bg, Color::Indexed(232));

        let mut buf = Buffer::empty(area);
        buf[(0, 0)]
            .set_fg(Color::Rgb(95, 0, 0))
            .set_bg(Color::Rgb(8, 8, 8));
        apply_color_mode(&mut buf, area, ColorMode::None);
        assert_eq!(buf[(0, 0)].fg, Color::Reset);
        assert_eq!(buf[(0, 0)].bg, Color::Reset);

        // Truecolour is a no-op.
        let mut buf = Buffer::empty(area);
        buf[(0, 0)].set_fg(Color::Rgb(95, 0, 0));
        apply_color_mode(&mut buf, area, ColorMode::Truecolor);
        assert_eq!(buf[(0, 0)].fg, Color::Rgb(95, 0, 0));
    }
}
