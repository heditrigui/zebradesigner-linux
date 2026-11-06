use crate::model::{BarcodeType, ElementData, Label, ShapeKind};

pub fn generate_epl(label: &Label, copies: u32) -> String {
    let dpmm = label.resolution.dots_per_mm();
    let width_dots = (label.width_mm * dpmm).round() as u32;
    let height_dots = (label.height_mm * dpmm).round() as u32;

    let mut epl = String::new();
    epl.push_str("N\n"); // Clear image buffer
    epl.push_str(&format!("q{}\n", width_dots)); // Set label width
    epl.push_str(&format!("Q{},24\n", height_dots)); // Set label length + 24 dot gap (~3mm)
    epl.push_str("ZT\n"); // Print from top

    for el in &label.elements {
        let x_dots = (el.x * dpmm).round().max(0.0) as u32;
        let y_dots = (el.y * dpmm).round().max(0.0) as u32;
        let w_dots = (el.width * dpmm).round().max(1.0) as u32;
        let h_dots = (el.height * dpmm).round().max(1.0) as u32;

        let rotation_code = match el.rotation {
            90 => 1,
            180 => 2,
            270 => 3,
            _ => 0,
        };

        match &el.data {
            ElementData::Text { text, font_size, bold: _, italic: _, color: _ } => {
                // EPL font selection:
                // 1: 8x12, 2: 10x16, 3: 12x20, 4: 14x24, 5: 32x48
                let (font_id, h_mult, v_mult) = if *font_size <= 9.0 {
                    (2, 1, 1)
                } else if *font_size <= 13.0 {
                    (3, 1, 1)
                } else if *font_size <= 18.0 {
                    (4, 1, 1)
                } else if *font_size <= 26.0 {
                    (4, 2, 2)
                } else {
                    let m = ((*font_size / 24.0).round() as u32).clamp(1, 4);
                    (5, m, m)
                };

                let clean_text = text.replace('"', "\\\"").replace('\n', " ");
                epl.push_str(&format!(
                    "A{},{},{},{},{},{},N,\"{}\"\n",
                    x_dots, y_dots, rotation_code, font_id, h_mult, v_mult, clean_text
                ));
            }
            ElementData::Barcode { barcode_type, data, show_text, module_width } => {
                let show_txt = if *show_text { 'B' } else { 'N' };
                let narrow_dots = ((*module_width * dpmm * 0.35).round() as u32).clamp(1, 8);
                let wide_dots = (narrow_dots * 2).clamp(2, 16);

                match barcode_type {
                    BarcodeType::QrCode => {
                        // b<x>,<y>,Q,m2,s<scale>,e<L/M/Q/H>,"<data>"
                        let scale = ((*module_width * 4.0).round() as u32).clamp(2, 9);
                        let clean_data = data.replace('"', "\\\"");
                        epl.push_str(&format!(
                            "b{},{},Q,m2,s{},eM,\"{}\"\n",
                            x_dots, y_dots, scale, clean_data
                        ));
                    }
                    BarcodeType::Code39 => {
                        // B<x>,<y>,<rot>,3,<narrow>,<wide>,<height>,<human_readable>,"<data>"
                        let clean_data = data.replace('"', "\\\"");
                        epl.push_str(&format!(
                            "B{},{},{},3,{},{},{},{},\"{}\"\n",
                            x_dots, y_dots, rotation_code, narrow_dots, wide_dots, h_dots, show_txt, clean_data
                        ));
                    }
                    _ => {
                        // B<x>,<y>,<rot>,1,<narrow>,<wide>,<height>,<human_readable>,"<data>" (Code 128)
                        let clean_data = data.replace('"', "\\\"");
                        epl.push_str(&format!(
                            "B{},{},{},1,{},{},{},{},\"{}\"\n",
                            x_dots, y_dots, rotation_code, narrow_dots, wide_dots, h_dots, show_txt, clean_data
                        ));
                    }
                }
            }
            ElementData::Shape { kind, stroke_width, .. } => {
                let s_dots = (*stroke_width * dpmm).round().max(1.0) as u32;
                match kind {
                    ShapeKind::Rectangle | ShapeKind::RoundedRectangle => {
                        // X<x>,<y>,<thickness>,<x2>,<y2> (Hollow Box)
                        let x2 = x_dots + w_dots;
                        let y2 = y_dots + h_dots;
                        epl.push_str(&format!("X{},{},{},{},{}\n", x_dots, y_dots, s_dots, x2, y2));
                    }
                    ShapeKind::Line => {
                        // LO<x>,<y>,<width>,<height> (Line Overlap / Solid bar)
                        epl.push_str(&format!("LO{},{},{},{}\n", x_dots, y_dots, w_dots, s_dots.max(1)));
                    }
                    ShapeKind::Ellipse => {
                        // Approximate with box in EPL
                        let x2 = x_dots + w_dots;
                        let y2 = y_dots + h_dots;
                        epl.push_str(&format!("X{},{},{},{},{}\n", x_dots, y_dots, s_dots, x2, y2));
                    }
                }
            }
            ElementData::Counter { prefix, current_value, padding_digits, font_size, .. } => {
                let formatted = format!("{}{:0width$}", prefix, current_value, width = padding_digits);
                let (font_id, h_mult, v_mult) = if *font_size <= 12.0 {
                    (3, 1, 1)
                } else if *font_size <= 18.0 {
                    (4, 1, 1)
                } else {
                    (5, 1, 1)
                };
                epl.push_str(&format!(
                    "A{},{},{},{},{},{},N,\"{}\"\n",
                    x_dots, y_dots, rotation_code, font_id, h_mult, v_mult, formatted
                ));
            }
            ElementData::DateTime { format_str, .. } => {
                let sample_date = format!("2026-11-06 12:30 ({})", format_str);
                epl.push_str(&format!(
                    "A{},{},{},3,1,1,N,\"{}\"\n",
                    x_dots, y_dots, rotation_code, sample_date
                ));
            }
        }
    }

    epl.push_str(&format!("P{}\n", copies.max(1))); // Print P1
    epl
}
