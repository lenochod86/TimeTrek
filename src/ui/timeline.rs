//! Časová osa: výpočty (zoom, posun, popisky osy, hledání položky pod kurzorem) a vykreslení
//! pruhů do pixelů. Pruhy kreslí Rust do bitmapy; Slint ji jen zobrazí (tisíce obdélníků jako
//! prvků UI by byly pomalé a paměťově drahé).

use super::fmt::{HOUR, MIN};

/// Jedna položka pruhu (úsek času s barvou).
#[derive(Debug, Clone)]
pub struct LaneItem {
    pub id: i64,
    pub start: i64,
    pub end: i64,
    pub color: [u8; 3],
    pub label: Option<String>,
    pub tip: String,
}

/// Zobrazený úsek časové osy v ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub start: i64,
    pub end: i64,
}

impl View {
    pub fn span(&self) -> i64 {
        (self.end - self.start).max(1)
    }
}

/// Nejkratší zobrazený úsek.
const MIN_SPAN: i64 = 2 * MIN;

/// Omezí úsek na hranice dne a na rozumnou délku.
pub fn clamp_view(day_start: i64, day_end: i64, start: i64, end: i64) -> View {
    let day_len = (day_end - day_start).max(MIN_SPAN);
    let len = (end - start).clamp(MIN_SPAN, day_len);
    let start = start.min(day_end - len).max(day_start);
    View { start, end: start + len }
}

/// Přiblížení / oddálení kolem bodu pod kurzorem (`frac` = poloha kurzoru 0..1).
pub fn zoom(view: View, day_start: i64, day_end: i64, frac: f64, zoom_out: bool) -> View {
    let span = view.span() as f64;
    let pivot = view.start as f64 + frac * span;
    let len = span * if zoom_out { 1.25 } else { 0.8 };
    let start = pivot - frac * len;
    clamp_view(day_start, day_end, start.round() as i64, (start + len).round() as i64)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    /// Poloha v rozsahu 0..1 přes šířku pruhu.
    pub frac: f64,
    pub label: String,
    pub major: bool,
}

/// Popisky osy; krok se volí tak, aby byly od sebe aspoň 64 px.
pub fn ticks(view: View, day_start: i64, width: f64, hm: impl Fn(i64) -> (u32, u32)) -> Vec<Tick> {
    const STEPS: [i64; 8] = [MIN, 5 * MIN, 10 * MIN, 15 * MIN, 30 * MIN, HOUR, 2 * HOUR, 6 * HOUR];
    let span = view.span() as f64;
    let px_per_ms = width.max(1.0) / span;
    let step = STEPS
        .iter()
        .copied()
        .find(|s| *s as f64 * px_per_ms >= 64.0)
        .unwrap_or(STEPS[STEPS.len() - 1]);
    let mut out = Vec::new();
    let first = ((view.start - day_start) as f64 / step as f64).ceil() as i64 * step + day_start;
    let mut t = first;
    while t <= view.end && out.len() < 200 {
        let (h, m) = hm(t);
        out.push(Tick { frac: (t - view.start) as f64 / span, label: format!("{h}:{m:02}"), major: m == 0 });
        t += step;
    }
    out
}

/// Položka pod časem `t`; u velmi krátkých položek se toleruje pár pixelů.
pub fn hit(items: &[LaneItem], t: f64, span: f64, width: f64) -> Option<&LaneItem> {
    items
        .iter()
        .rev()
        .find(|it| t >= it.start as f64 && t <= it.end as f64)
        .or_else(|| {
            let tol = 3.0 * span / width.max(1.0);
            items.iter().find(|it| t >= it.start as f64 - tol && t <= it.end as f64 + tol)
        })
}

/// RGBA bitmapa, průhledná, kde nic není.
pub struct Bitmap {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Bitmap {
    fn new(w: u32, h: u32) -> Self {
        Self { w, h, rgba: vec![0; (w * h * 4) as usize] }
    }

    fn put(&mut self, x: u32, y: u32, c: [u8; 3], alpha: f32) {
        let i = ((y * self.w + x) * 4) as usize;
        if alpha >= 1.0 {
            self.rgba[i..i + 3].copy_from_slice(&c);
            self.rgba[i + 3] = 255;
            return;
        }
        // Přimíchání k tomu, co tam je (předpokládá se, že je pod tím pozadí pruhu).
        let dst_a = self.rgba[i + 3] as f32 / 255.0;
        let out_a = alpha + dst_a * (1.0 - alpha);
        for (k, src) in c.iter().enumerate() {
            let d = self.rgba[i + k] as f32;
            let v = (*src as f32 * alpha + d * dst_a * (1.0 - alpha)) / out_a.max(0.0001);
            self.rgba[i + k] = v.round().clamp(0.0, 255.0) as u8;
        }
        self.rgba[i + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    }

    fn fill(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, c: [u8; 3], alpha: f32) {
        let (x0, x1) = (x0.max(0), x1.min(self.w as i64));
        let (y0, y1) = (y0.max(0), y1.min(self.h as i64));
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x as u32, y as u32, c, alpha);
            }
        }
    }

    fn frame(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, thick: i64, c: [u8; 3]) {
        self.fill(x0, y0, x1, y0 + thick, c, 1.0);
        self.fill(x0, y1 - thick, x1, y1, c, 1.0);
        self.fill(x0, y0, x0 + thick, y1, c, 1.0);
        self.fill(x1 - thick, y0, x1, y1, c, 1.0);
    }
}

fn px_range(start: i64, end: i64, view: View, w: u32) -> (i64, i64) {
    let k = w as f64 / view.span() as f64;
    let x0 = ((start - view.start) as f64 * k).floor() as i64;
    let x1 = (((end - view.start) as f64 * k).ceil() as i64).max(x0 + 1);
    (x0, x1)
}

const SELECT_COLOR: [u8; 3] = [255, 255, 255];
const DRAG_COLOR: [u8; 3] = [80, 220, 160];
/// Průhlednost černého oddělovače mezi úseky.
const SEPARATOR_ALPHA: f32 = 0.4;

/// Vykreslí jeden pruh. `scale` je poměr fyzických pixelů k logickým (ohraničení výběru).
pub fn render_lane(
    items: &[LaneItem],
    w: u32,
    h: u32,
    view: View,
    selected: Option<i64>,
    drag: Option<(i64, i64)>,
    scale: f32,
) -> Bitmap {
    let mut bmp = Bitmap::new(w.max(1), h.max(1));
    let border = (2.0 * scale).round().max(1.0) as i64;
    let sep = scale.round().max(1.0) as i64;
    for it in items {
        if it.end < view.start || it.start > view.end {
            continue;
        }
        let (x0, x1) = px_range(it.start, it.end, view, bmp.w);
        bmp.fill(x0, 0, x1, bmp.h as i64, it.color, 1.0);
        // Jemný tmavý oddělovač na začátku úseku: bez něj by se sousední záznamy stejné barvy
        // (např. změna titulku v téže aplikaci) slily a nebylo by vidět, kde začínají a končí.
        if x1 - x0 >= 3 * sep {
            bmp.fill(x0, 0, x0 + sep, bmp.h as i64, [0, 0, 0], SEPARATOR_ALPHA);
        }
        if selected == Some(it.id) && x1 - x0 > 2 * border {
            bmp.frame(x0, 0, x1, bmp.h as i64, border, SELECT_COLOR);
        }
    }
    if let Some((a, b)) = drag {
        let (a, b) = (a.min(b), a.max(b));
        let (x0, x1) = px_range(a, b, view, bmp.w);
        bmp.fill(x0, 0, x1, bmp.h as i64, DRAG_COLOR, 0.35);
        bmp.frame(x0, 0, x1, bmp.h as i64, scale.round().max(1.0) as i64, DRAG_COLOR);
    }
    bmp
}

/// Přehled celého dne pod osou: zelené značky aktivity u spodního okraje.
pub fn render_overview(active: &[(i64, i64)], w: u32, h: u32, day_start: i64, day_len: i64, scale: f32) -> Bitmap {
    let mut bmp = Bitmap::new(w.max(1), h.max(1));
    let view = View { start: day_start, end: day_start + day_len.max(1) };
    let bar = (8.0 * scale).round() as i64;
    for &(s, e) in active {
        let (x0, x1) = px_range(s, e, view, bmp.w);
        bmp.fill(x0, bmp.h as i64 - bar, x1, bmp.h as i64, [76, 154, 31], 0.55);
    }
    bmp
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 24 * HOUR;

    fn item(id: i64, start: i64, end: i64) -> LaneItem {
        LaneItem { id, start, end, color: [10, 20, 30], label: None, tip: String::new() }
    }

    #[test]
    fn clamp_keeps_view_inside_day_and_min_span() {
        let v = clamp_view(0, DAY, -5 * HOUR, 2 * HOUR);
        assert_eq!(v, View { start: 0, end: 7 * HOUR });
        let v = clamp_view(0, DAY, DAY - HOUR, DAY + 5 * HOUR);
        assert_eq!(v.end, DAY);
        let v = clamp_view(0, DAY, 100, 200);
        assert_eq!(v.end - v.start, 2 * MIN);
        let v = clamp_view(0, DAY, -DAY, 3 * DAY);
        assert_eq!(v, View { start: 0, end: DAY });
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let v = View { start: 8 * HOUR, end: 12 * HOUR };
        let z = zoom(v, 0, DAY, 0.5, false);
        assert_eq!(z.span(), (4.0 * HOUR as f64 * 0.8) as i64);
        assert_eq!(z.start + z.span() / 2, 10 * HOUR);
        let out = zoom(v, 0, DAY, 0.0, true);
        assert_eq!(out.start, v.start);
        assert!(out.span() > v.span());
    }

    #[test]
    fn ticks_choose_step_by_width() {
        let v = View { start: 0, end: 2 * HOUR };
        let t = ticks(v, 0, 1200.0, |m| ((m / HOUR) as u32, ((m % HOUR) / MIN) as u32));
        // 1200 px na 2 h = 10 px/min, 64 px -> krok 10 min
        assert_eq!(t[1].label, "0:10");
        assert!(t[0].major && !t[1].major);
        assert_eq!(t.len(), 13);
        assert!((t[6].frac - 0.5).abs() < 1e-9);
    }

    #[test]
    fn hit_prefers_last_and_tolerates_short_items() {
        let items = vec![item(1, 0, 100), item(2, 50, 150), item(3, 1000, 1001)];
        assert_eq!(hit(&items, 75.0, 2000.0, 1000.0).unwrap().id, 2);
        assert_eq!(hit(&items, 1003.0, 2000.0, 1000.0).unwrap().id, 3); // v toleranci 6 ms
        assert!(hit(&items, 500.0, 2000.0, 1000.0).is_none());
    }

    #[test]
    fn lane_draws_items_selection_and_drag() {
        let view = View { start: 0, end: 100 };
        let items = vec![item(1, 10, 30), item(2, 60, 62)];
        let b = render_lane(&items, 100, 6, view, Some(1), None, 1.0);
        let px = |x: u32, y: u32| &b.rgba[((y * 100 + x) * 4) as usize..][..4];
        assert_eq!(px(5, 1), &[0, 0, 0, 0]); // mimo položku průhledné
        assert_eq!(px(20, 3), &[10, 20, 30, 255]); // uvnitř položky
        assert_eq!(px(10, 3), &[255, 255, 255, 255]); // rámeček výběru
        assert_eq!(px(20, 1), &[255, 255, 255, 255]);
        assert_eq!(px(61, 1), &[10, 20, 30, 255]); // krátká položka je aspoň 1 px široká
        let d = render_lane(&[], 100, 4, view, None, Some((40, 20)), 1.0);
        let dp = &d.rgba[((100 + 30) * 4) as usize..][..4];
        assert!(dp[3] > 0 && dp[3] < 255);
    }

    #[test]
    fn adjacent_items_of_same_color_keep_a_visible_boundary() {
        let view = View { start: 0, end: 100 };
        let items = vec![item(1, 0, 50), item(2, 50, 100)];
        let b = render_lane(&items, 100, 4, view, None, None, 1.0);
        let px = |x: u32| &b.rgba[((2 * 100 + x) * 4) as usize..][..4];
        assert_eq!(px(25), &[10, 20, 30, 255]); // uvnitř úseku plná barva
        assert_eq!(px(50), &[6, 12, 18, 255]); // začátek druhého úseku je ztmavený
        assert_eq!(px(0), &[6, 12, 18, 255]);
        assert_eq!(px(51), &[10, 20, 30, 255]);
        // Úsek užší než 3 px se neděli, ať z něj nezůstane jen čárka.
        let b = render_lane(&[item(1, 10, 12)], 100, 4, view, None, None, 1.0);
        assert_eq!(&b.rgba[((2 * 100 + 10) * 4) as usize..][..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn overview_marks_only_bottom_strip() {
        let b = render_overview(&[(0, DAY / 2)], 24, 30, 0, DAY, 1.0);
        let a = |x: u32, y: u32| b.rgba[((y * 24 + x) * 4 + 3) as usize];
        assert!(a(3, 29) > 0);
        assert_eq!(a(3, 5), 0);
        assert_eq!(a(20, 29), 0);
    }
}
