use crate::barcode::{bar_dots, bar_height_dots, generate_code128, generate_code39, qr_scale};
use crate::model::{along_across, counter_text, format_datetime, prints_black, BarcodeType, ElementData, Label, ShapeKind};

/// Builds a complete print job. Labels with counters get one form per copy
/// so each copy carries its own serial number.
pub fn generate_epl(label: &Label, copies: u32) -> String {
    let copies = copies.max(1);
    if label.has_counters() {
        (0..copies).map(|i| render_form(label, i as i64, 1)).collect()
    } else {
        render_form(label, 0, copies)
    }
}

fn render_form(label: &Label, counter_offset: i64, quantity: u32) -> String {
    let dpmm = label.resolution.dots_per_mm();
    let width_dots = (label.width_mm * dpmm).round() as u32;
    let height_dots = (label.height_mm * dpmm).round() as u32;

    let mut epl = String::new();
    epl.push_str("N\n"); // Clear image buffer
    epl.push_str(&format!("q{}\n", width_dots)); // Set label width
    epl.push_str(&format!("Q{},24\n", height_dots)); // Set label length + 24 dot gap (~3mm)
    epl.push_str("ZT\n"); // Print from top

    for el in &label.elements {
        let x = (el.x * dpmm).round().max(0.0) as u32;
        let y = (el.y * dpmm).round().max(0.0) as u32;
        let w = (el.width * dpmm).round().max(1.0) as u32;
        let h = (el.height * dpmm).round().max(1.0) as u32;

        // EPL rotates around the field origin, so start from the matching box corner
        let (rot_code, ox, oy) = match el.rotation {
            90 => (1, x + w, y),
            180 => (2, x + w, y + h),
            270 => (3, x, y + h),
            _ => (0, x, y),
        };

        match &el.data {
            ElementData::Text { text, font_size, bold, color, .. } => {
                let reverse = !prints_black(*color);
                push_text(&mut epl, ox, oy, rot_code, *font_size, dpmm, *bold, reverse, text);
            }
            ElementData::Counter { prefix, current_value, step, padding_digits, font_size, bold } => {
                let value = current_value + step * counter_offset;
                let formatted = counter_text(prefix, value, *padding_digits);
                push_text(&mut epl, ox, oy, rot_code, *font_size, dpmm, *bold, false, &formatted);
            }
            ElementData::DateTime { format_str, font_size, bold } => {
                let formatted = format_datetime(format_str);
                push_text(&mut epl, ox, oy, rot_code, *font_size, dpmm, *bold, false, &formatted);
            }
            ElementData::Barcode { barcode_type, data, show_text, .. } => {
                if *barcode_type == BarcodeType::QrCode {
                    // b<x>,<y>,Q,m2,s<scale>,e<L/M/Q/H>,"<data>"
                    let scale = qr_scale(data, w.min(h));
                    epl.push_str(&format!("b{},{},Q,m2,s{},eM,\"{}\"\n", x, y, scale, escape_quoted(data)));
                    continue;
                }

                let (along, across) = along_across(el.rotation, w, h);
                let bar_h = bar_height_dots(across, *show_text);
                let hr = if *show_text { 'B' } else { 'N' };
                // B<x>,<y>,<rot>,<type>,<narrow>,<wide>,<height>,<human_readable>,"<data>"
                let (symbology, narrow, payload) = match barcode_type {
                    BarcodeType::Code39 => (
                        "3",
                        bar_dots(generate_code39(data).modules.len(), along),
                        data.trim_matches('*').to_ascii_uppercase(),
                    ),
                    // Subset B matches the module count the box was sized for
                    _ => ("1B", bar_dots(generate_code128(data).modules.len(), along), data.clone()),
                };
                epl.push_str(&format!(
                    "B{},{},{},{},{},{},{},{},\"{}\"\n",
                    ox, oy, rot_code, symbology, narrow, narrow * 3, bar_h, hr, escape_quoted(&payload)
                ));
            }
            ElementData::Shape { kind, stroke_width, fill_color, .. } => {
                let stroke = (*stroke_width * dpmm).round().max(1.0) as u32;
                let filled = fill_color.map_or(false, prints_black);
                match kind {
                    ShapeKind::Line => {
                        // LO<x>,<y>,<width>,<height> (solid bar)
                        if el.rotation == 90 || el.rotation == 270 {
                            epl.push_str(&format!("LO{},{},{},{}\n", x, y, stroke, h));
                        } else {
                            epl.push_str(&format!("LO{},{},{},{}\n", x, y, w, stroke));
                        }
                    }
                    // EPL has no rounded corners or ellipses, so they print as plain boxes
                    ShapeKind::Rectangle | ShapeKind::RoundedRectangle | ShapeKind::Ellipse => {
                        if filled {
                            epl.push_str(&format!("LO{},{},{},{}\n", x, y, w, h));
                        } else {
                            // X<x>,<y>,<thickness>,<x2>,<y2> (hollow box)
                            epl.push_str(&format!("X{},{},{},{},{}\n", x, y, stroke, x + w, y + h));
                        }
                    }
                }
            }
        }
    }

    epl.push_str(&format!("P{}\n", quantity.max(1)));
    epl
}

#[allow(clippy::too_many_arguments)]
fn push_text(epl: &mut String, x: u32, y: u32, rot: u8, font_size: f32, dpmm: f32, bold: bool, reverse: bool, text: &str) {
    let (font, mult) = pick_font(font_size, dpmm);
    let style = if reverse { 'R' } else { 'N' };
    let escaped = escape_quoted(text);
    // EPL has no bold, so bold is struck twice one dot apart
    let strikes = if bold { 2 } else { 1 };
    for offset in 0..strikes {
        epl.push_str(&format!(
            "A{},{},{},{},{},{},{},\"{}\"\n",
            x + offset, y, rot, font, mult, mult, style, escaped
        ));
    }
}

/// Picks the fixed font and multiplier closest to the requested point size.
/// Font heights are the 203 DPI values.
fn pick_font(font_size: f32, dpmm: f32) -> (u8, u32) {
    const FONT_HEIGHTS: [(u8, f32); 5] = [(1, 12.0), (2, 16.0), (3, 20.0), (4, 24.0), (5, 48.0)];
    let target = font_size * dpmm * 0.35;
    let mut best = (2, 1);
    let mut best_diff = f32::MAX;
    for (font, height) in FONT_HEIGHTS {
        for mult in 1..=4u32 {
            let diff = (height * mult as f32 - target).abs();
            if diff < best_diff {
                best_diff = diff;
                best = (font, mult);
            }
        }
    }
    best
}

fn escape_quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Element, Label};

    fn label_with(data: ElementData, rotation: i32) -> Label {
        let mut label = Label::new_empty(50.0, 25.0, "test");
        label.elements.push(Element {
            id: 1,
            name: "e".to_string(),
            x: 2.0,
            y: 3.0,
            width: 40.0,
            height: 10.0,
            rotation,
            data,
        });
        label
    }

    fn text(content: &str, bold: bool, color: [u8; 4]) -> ElementData {
        ElementData::Text { text: content.to_string(), font_size: 12.0, bold, italic: false, color }
    }

    #[test]
    fn copies_appear_once_in_the_job() {
        let epl = generate_epl(&label_with(text("A", false, [0, 0, 0, 255]), 0), 5);
        assert_eq!(epl.matches("\nN\n").count() + usize::from(epl.starts_with("N\n")), 1);
        assert!(epl.ends_with("P5\n"), "{epl}");
    }

    #[test]
    fn counters_get_one_form_per_copy() {
        let data = ElementData::Counter { prefix: "SN-".to_string(), current_value: 7, step: 2, padding_digits: 3, font_size: 10.0, bold: false };
        let epl = generate_epl(&label_with(data, 0), 3);
        assert_eq!(epl.matches("P1\n").count(), 3);
        for serial in ["SN-007", "SN-009", "SN-011"] {
            assert!(epl.contains(serial), "missing {serial} in {epl}");
        }
    }

    #[test]
    fn date_prints_without_format_text() {
        let data = ElementData::DateTime { format_str: "%Y".to_string(), font_size: 9.0, bold: false };
        let epl = generate_epl(&label_with(data, 0), 1);
        assert!(!epl.contains('%'), "{epl}");
    }

    #[test]
    fn rotation_starts_from_box_corner() {
        let epl = generate_epl(&label_with(text("A", false, [0, 0, 0, 255]), 90), 1);
        // x 16 + width 320 = 336; y 24; rotation code 1
        assert!(epl.contains("A336,24,1,"), "{epl}");
    }

    #[test]
    fn quotes_and_backslashes_are_escaped() {
        let epl = generate_epl(&label_with(text("a\"b\\c", false, [0, 0, 0, 255]), 0), 1);
        assert!(epl.contains("\"a\\\"b\\\\c\""), "{epl}");
    }

    #[test]
    fn bold_strikes_twice_and_light_text_reverses() {
        let epl = generate_epl(&label_with(text("A", true, [255, 255, 255, 255]), 0), 1);
        assert_eq!(epl.matches(",R,\"A\"").count(), 2);
    }

    #[test]
    fn dark_fill_prints_solid() {
        let data = ElementData::Shape {
            kind: ShapeKind::Rectangle,
            stroke_width: 0.5,
            stroke_color: [0, 0, 0, 255],
            fill_color: Some([0, 0, 0, 255]),
            corner_radius: 0.0,
        };
        assert!(generate_epl(&label_with(data, 0), 1).contains("LO16,24,320,80\n"));
    }

    #[test]
    fn font_size_maps_to_nearest_height() {
        // 14pt at 8 dots/mm is about 39 dots: font 3 (20 dots) doubled
        assert_eq!(pick_font(14.0, 8.0), (3, 2));
    }
}
