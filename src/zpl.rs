use crate::model::{BarcodeType, ElementData, Label, ShapeKind};

pub fn generate_zpl(label: &Label) -> String {
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
        let x_dots = (el.x * dpmm).round() as i32;
        let y_dots = (el.y * dpmm).round() as i32;
        let w_dots = (el.width * dpmm).round() as u32;
        let h_dots = (el.height * dpmm).round() as u32;

        match &el.data {
            ElementData::Text { text, font_size, bold: _, italic: _, color: _ } => {
                let font_h = (*font_size * dpmm * 0.35).round() as u32;
                let font_w = (font_h as f32 * 0.85).round() as u32;
                zpl.push_str(&format!("^FO{},{}^A0N,{},{}^FD{}^FS\n", x_dots, y_dots, font_h.max(16), font_w.max(12), text));
            }
            ElementData::Barcode { barcode_type, data, show_text, module_width } => {
                let show_txt_char = if *show_text { 'Y' } else { 'N' };
                let bar_ratio = ((*module_width * dpmm * 0.35).round() as u32).clamp(1, 10);
                match barcode_type {
                    BarcodeType::Code128 => {
                        zpl.push_str(&format!(
                            "^FO{},{}^BY{},3,{}^BCN,{},{},N,N^FD{}^FS\n",
                            x_dots, y_dots, bar_ratio, h_dots, h_dots, show_txt_char, data
                        ));
                    }
                    BarcodeType::Code39 => {
                        zpl.push_str(&format!(
                            "^FO{},{}^BY{},3,{}^B3N,N,{},{},N^FD{}^FS\n",
                            x_dots, y_dots, bar_ratio, h_dots, h_dots, show_txt_char, data
                        ));
                    }
                    BarcodeType::QrCode => {
                        let mag = ((*module_width * 3.0).round() as u32).clamp(2, 10);
                        zpl.push_str(&format!(
                            "^FO{},{}^BQN,2,{}^FDQA,{}^FS\n",
                            x_dots, y_dots, mag, data
                        ));
                    }
                    _ => {
                        zpl.push_str(&format!(
                            "^FO{},{}^BY{},3,{}^BCN,{},{},N,N^FD{}^FS\n",
                            x_dots, y_dots, bar_ratio, h_dots, h_dots, show_txt_char, data
                        ));
                    }
                }
            }
            ElementData::Shape { kind, stroke_width, corner_radius, .. } => {
                let stroke_dots = (*stroke_width * dpmm).round().max(1.0) as u32;
                match kind {
                    ShapeKind::Rectangle => {
                        zpl.push_str(&format!("^FO{},{}^GB{},{},{}^FS\n", x_dots, y_dots, w_dots, h_dots, stroke_dots));
                    }
                    ShapeKind::RoundedRectangle => {
                        let r_idx = ((*corner_radius * 0.5).round() as u32).clamp(1, 8);
                        zpl.push_str(&format!("^FO{},{}^GB{},{},{},B,{}^FS\n", x_dots, y_dots, w_dots, h_dots, stroke_dots, r_idx));
                    }
                    ShapeKind::Line => {
                        let thick = stroke_dots.max(1);
                        zpl.push_str(&format!("^FO{},{}^GB{},{},{}^FS\n", x_dots, y_dots, w_dots, thick, thick));
                    }
                    ShapeKind::Ellipse => {
                        zpl.push_str(&format!("^FO{},{}^GE{},{},{}^FS\n", x_dots, y_dots, w_dots, h_dots, stroke_dots));
                    }
                }
            }
            ElementData::Counter { prefix, current_value, padding_digits, font_size, .. } => {
                let formatted = format!("{}{:0width$}", prefix, current_value, width = padding_digits);
                let font_h = (*font_size * dpmm * 0.35).round() as u32;
                let font_w = (font_h as f32 * 0.85).round() as u32;
                zpl.push_str(&format!("^FO{},{}^A0N,{},{}^FD{}^FS\n", x_dots, y_dots, font_h.max(16), font_w.max(12), formatted));
            }
            ElementData::DateTime { format_str, font_size, .. } => {
                let sample_date = "2026-11-06 12:30";
                let font_h = (*font_size * dpmm * 0.35).round() as u32;
                let font_w = (font_h as f32 * 0.85).round() as u32;
                zpl.push_str(&format!("^FO{},{}^A0N,{},{}^FD{} ({})^FS\n", x_dots, y_dots, font_h.max(16), font_w.max(12), sample_date, format_str));
            }
        }
    }

    zpl.push_str("^PQ1,0,1,Y\n"); // Print quantity: 1
    zpl.push_str("^XZ\n");
    zpl
}
