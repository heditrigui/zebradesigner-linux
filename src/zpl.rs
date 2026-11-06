use crate::barcode::{bar_dots, bar_height_dots, generate_code128, generate_code39, qr_scale};
use crate::model::{along_across, counter_text, format_datetime, prints_black, BarcodeType, ElementData, Label, ShapeKind};

/// Builds a complete print job. Labels with counters get one format per copy
/// so each copy carries its own serial number.
pub fn generate_zpl(label: &Label, copies: u32) -> String {
    let copies = copies.max(1);
    if label.has_counters() {
        (0..copies).map(|i| render_format(label, i as i64, 1)).collect()
    } else {
        render_format(label, 0, copies)
    }
}

fn render_format(label: &Label, counter_offset: i64, quantity: u32) -> String {
    let dpmm = label.resolution.dots_per_mm();
    let width_dots = (label.width_mm * dpmm).round() as u32;
    let height_dots = (label.height_mm * dpmm).round() as u32;

    let mut zpl = String::new();
    zpl.push_str("^XA\n");
    zpl.push_str(&format!("^PW{}\n", width_dots));
    zpl.push_str(&format!("^LL{}\n", height_dots));
    zpl.push_str("^LH0,0\n");
    zpl.push_str("^CI28\n"); // UTF-8 character encoding

    for el in &label.elements {
        let x = (el.x * dpmm).round() as i32;
        let y = (el.y * dpmm).round() as i32;
        let w = (el.width * dpmm).round().max(1.0) as i32;
        let h = (el.height * dpmm).round().max(1.0) as i32;
        let field = TextField { x, y, w, h, rotation: el.rotation, dpmm };

        match &el.data {
            ElementData::Text { text, font_size, bold, color, .. } => {
                field.push(&mut zpl, text, *font_size, *bold, !prints_black(*color));
            }
            ElementData::Counter { prefix, current_value, step, padding_digits, font_size, bold } => {
                let value = current_value + step * counter_offset;
                field.push(&mut zpl, &counter_text(prefix, value, *padding_digits), *font_size, *bold, false);
            }
            ElementData::DateTime { format_str, font_size, bold } => {
                field.push(&mut zpl, &format_datetime(format_str), *font_size, *bold, false);
            }
            ElementData::Barcode { barcode_type, data, show_text, .. } => {
                if *barcode_type == BarcodeType::QrCode {
                    // ^BQ only supports normal orientation
                    let mag = qr_scale(data, w.min(h) as u32);
                    zpl.push_str(&format!("^FO{},{}^BQN,2,{}^FH^FDMA,{}^FS\n", x, y, mag, escape_field(data)));
                    continue;
                }

                let (along, across) = along_across(el.rotation, w as u32, h as u32);
                let bar_h = bar_height_dots(across, *show_text);
                let origin = field_origin(x, y, w, h, el.rotation, bar_h as i32);
                let o = orientation(el.rotation);
                let hr = if *show_text { 'Y' } else { 'N' };
                match barcode_type {
                    BarcodeType::Code39 => {
                        let dots = bar_dots(generate_code39(data).modules.len(), along);
                        let payload = data.trim_matches('*').to_ascii_uppercase();
                        zpl.push_str(&format!(
                            "{}^BY{},3,{}^B3{},N,{},{},N^FH^FD{}^FS\n",
                            origin, dots, bar_h, o, bar_h, hr, escape_field(&payload)
                        ));
                    }
                    _ => {
                        let dots = bar_dots(generate_code128(data).modules.len(), along);
                        // '>' introduces a subset switch inside ^BC
                        let payload = escape_field(data).replace('>', "><");
                        zpl.push_str(&format!(
                            "{}^BY{},3,{}^BC{},{},{},N,N^FH^FD{}^FS\n",
                            origin, dots, bar_h, o, bar_h, hr, payload
                        ));
                    }
                }
            }
            ElementData::Shape { kind, stroke_width, fill_color, corner_radius, .. } => {
                let stroke = (*stroke_width * dpmm).round().max(1.0) as i32;
                let filled = fill_color.map_or(false, prints_black);
                let thickness = if filled { w.min(h) } else { stroke.min(w.min(h)) };
                match kind {
                    ShapeKind::Rectangle => {
                        zpl.push_str(&format!("^FO{},{}^GB{},{},{}^FS\n", x, y, w, h, thickness));
                    }
                    ShapeKind::RoundedRectangle => {
                        // ZPL rounding is 0-8, as a fraction of half the shorter side
                        let radius = *corner_radius * dpmm;
                        let rounding = ((radius * 16.0 / w.min(h) as f32).round() as i32).clamp(0, 8);
                        zpl.push_str(&format!("^FO{},{}^GB{},{},{},B,{}^FS\n", x, y, w, h, thickness, rounding));
                    }
                    ShapeKind::Line => {
                        if el.rotation == 90 || el.rotation == 270 {
                            zpl.push_str(&format!("^FO{},{}^GB{},{},{}^FS\n", x, y, stroke, h, stroke));
                        } else {
                            zpl.push_str(&format!("^FO{},{}^GB{},{},{}^FS\n", x, y, w, stroke, stroke));
                        }
                    }
                    ShapeKind::Ellipse => {
                        zpl.push_str(&format!("^FO{},{}^GE{},{},{}^FS\n", x, y, w, h, thickness));
                    }
                }
            }
        }
    }

    zpl.push_str(&format!("^PQ{},0,1,Y\n", quantity.max(1)));
    zpl.push_str("^XZ\n");
    zpl
}

struct TextField {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rotation: i32,
    dpmm: f32,
}

impl TextField {
    fn push(&self, zpl: &mut String, text: &str, font_size: f32, bold: bool, reverse: bool) {
        let font_h = ((font_size * self.dpmm * 0.35).round() as i32).max(16);
        let font_w = ((font_h as f32 * 0.85).round() as i32).max(12);
        let o = orientation(self.rotation);
        let fr = if reverse { "^FR" } else { "" };
        // ZPL has no bold for the scalable font, so bold is struck twice one dot apart
        let strikes = if bold { 2 } else { 1 };
        for offset in 0..strikes {
            let origin = field_origin(self.x + offset, self.y, self.w, self.h, self.rotation, font_h);
            zpl.push_str(&format!(
                "{}{}^A0{},{},{}^FH^FD{}^FS\n",
                origin, fr, o, font_h, font_w, escape_field(text)
            ));
        }
    }
}

fn orientation(rotation: i32) -> char {
    match rotation {
        90 => 'R',
        180 => 'I',
        270 => 'B',
        _ => 'N',
    }
}

/// Places a field so its rotated content starts from the same corner of the
/// element box as the on-screen preview. `cross` is the field's height in dots.
fn field_origin(x: i32, y: i32, w: i32, h: i32, rotation: i32, cross: i32) -> String {
    match rotation {
        90 => format!("^FT{},{}", x + w - cross, y),
        180 => format!("^FT{},{}", x + w, y + h - cross),
        270 => format!("^FT{},{}", x + cross, y + h),
        _ => format!("^FO{},{}", x, y),
    }
}

/// Encodes characters that ZPL would treat as commands, for use after ^FH.
fn escape_field(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '^' => out.push_str("_5E"),
            '~' => out.push_str("_7E"),
            '_' => out.push_str("_5F"),
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
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

    fn counter() -> ElementData {
        ElementData::Counter { prefix: "SN-".to_string(), current_value: 7, step: 2, padding_digits: 3, font_size: 10.0, bold: false }
    }

    #[test]
    fn copies_use_print_quantity() {
        let zpl = generate_zpl(&label_with(text("A", false, [0, 0, 0, 255]), 0), 5);
        assert_eq!(zpl.matches("^XA").count(), 1);
        assert!(zpl.contains("^PQ5,0,1,Y"));
    }

    #[test]
    fn counters_get_one_format_per_copy() {
        let zpl = generate_zpl(&label_with(counter(), 0), 3);
        assert_eq!(zpl.matches("^XA").count(), 3);
        assert_eq!(zpl.matches("^PQ1,").count(), 3);
        for serial in ["SN-007", "SN-009", "SN-011"] {
            assert!(zpl.contains(serial), "missing {serial} in {zpl}");
        }
    }

    #[test]
    fn command_characters_are_escaped() {
        let zpl = generate_zpl(&label_with(text("a^b~c_d", false, [0, 0, 0, 255]), 0), 1);
        assert!(zpl.contains("^FDa_5Eb_7Ec_5Fd^FS"), "{zpl}");
    }

    #[test]
    fn rotation_sets_orientation() {
        let zpl = generate_zpl(&label_with(text("A", false, [0, 0, 0, 255]), 90), 1);
        assert!(zpl.contains("^A0R,"), "{zpl}");
        assert!(zpl.contains("^FT"), "{zpl}");
    }

    #[test]
    fn bold_strikes_twice_and_light_text_reverses() {
        let zpl = generate_zpl(&label_with(text("A", true, [255, 255, 255, 255]), 0), 1);
        assert_eq!(zpl.matches("^FDA^FS").count(), 2);
        assert!(zpl.contains("^FR"), "{zpl}");
    }

    #[test]
    fn dark_fill_is_solid_and_light_fill_is_outline() {
        let shape = |fill| ElementData::Shape {
            kind: ShapeKind::Rectangle,
            stroke_width: 0.5,
            stroke_color: [0, 0, 0, 255],
            fill_color: Some(fill),
            corner_radius: 0.0,
        };
        assert!(generate_zpl(&label_with(shape([0, 0, 0, 255]), 0), 1).contains("^GB320,80,80^FS"));
        assert!(generate_zpl(&label_with(shape([240, 240, 240, 255]), 0), 1).contains("^GB320,80,4^FS"));
    }

    #[test]
    fn barcode_fits_its_box() {
        let data = ElementData::Barcode {
            barcode_type: BarcodeType::Code128,
            data: "ABC123".to_string(),
            show_text: true,
            module_width: 0.4,
        };
        let zpl = generate_zpl(&label_with(data, 0), 1);
        // 6 chars + start + check + stop = 101 modules in 320 dots -> 3 dots each; 80% of 80 dots tall
        assert!(zpl.contains("^BY3,3,64^BCN,64,Y,N,N^FH^FDABC123^FS"), "{zpl}");
    }
}
