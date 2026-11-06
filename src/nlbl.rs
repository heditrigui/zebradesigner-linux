use crate::model::{BarcodeType, Element, ElementData, Label, Orientation, Resolution, ShapeKind};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

const NLBL_PASSWORD: &str = ",^_A5Fus&!?j='Epiq*e";

pub fn import_nlbl(path: &Path) -> Result<Label, String> {
    if !path.exists() {
        return Err(format!("File does not exist: {}", path.display()));
    }

    let temp_dir = std::env::temp_dir().join(format!("nlbl_import_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir);

    // 1. Try extracting with 7z using the standard NiceLabel / ZebraDesigner password
    let status_encrypted = Command::new("7z")
        .arg("x")
        .arg(path)
        .arg(format!("-p{}", NLBL_PASSWORD))
        .arg(format!("-o{}", temp_dir.display()))
        .arg("-y")
        .output();

    let extracted = match status_encrypted {
        Ok(out) if out.status.success() => true,
        _ => {
            // Fallback: try unencrypted extract
            let status_plain = Command::new("7z")
                .arg("x")
                .arg(path)
                .arg(format!("-o{}", temp_dir.display()))
                .arg("-y")
                .output();
            matches!(status_plain, Ok(out) if out.status.success())
        }
    };

    if !extracted {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(format!("Failed to extract archive: {}", path.display()));
    }

    // 2. Parse variables from .slnx solution file if present
    let mut variables = HashMap::new();
    if let Ok(entries) = fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |ext| ext == "slnx") {
                if let Ok(content) = fs::read_to_string(&p) {
                    parse_variables(&content, &mut variables);
                }
            }
        }
    }

    // 3. Find and parse format definition XML file
    let formats_dir = temp_dir.join("Formats");
    let mut format_file = None;

    if formats_dir.exists() {
        if let Ok(entries) = fs::read_dir(&formats_dir) {
            for entry in entries.flatten() {
                format_file = Some(entry.path());
                break;
            }
        }
    }

    let format_path = format_file.ok_or_else(|| {
        let _ = fs::remove_dir_all(&temp_dir);
        "Formats/ folder not found inside .nlbl package".to_string()
    })?;

    let format_xml = fs::read_to_string(&format_path).map_err(|e| {
        let _ = fs::remove_dir_all(&temp_dir);
        format!("Failed to read format file: {}", e)
    })?;

    let label = parse_format_xml(&format_xml, &variables, path)?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(label)
}

fn parse_variables(xml: &str, vars: &mut HashMap<String, String>) {
    // Look for <Item Type="Variable"> ... <Name>...</Name> ... <StringValue>...</StringValue>
    let mut rest = xml;
    while let Some(start_idx) = rest.find("<Item Type=\"Variable\">") {
        rest = &rest[start_idx + 22..];
        let end_idx = rest.find("</Item>").unwrap_or(rest.len());
        let item_xml = &rest[..end_idx];

        let name = extract_tag_value(item_xml, "Name");
        let val = extract_tag_value(item_xml, "StringValue")
            .or_else(|| extract_tag_value(item_xml, "UserValue"));

        if let (Some(n), Some(v)) = (name, val) {
            vars.insert(n, v);
        }
        rest = &rest[end_idx..];
    }
}

fn parse_format_xml(
    xml: &str,
    variables: &HashMap<String, String>,
    source_path: &Path,
) -> Result<Label, String> {
    let file_stem = source_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported Label".to_string());

    // Extract Media Dimensions
    let width_mm = extract_media_dimension(xml, "Width").unwrap_or(101.6);
    let height_mm = extract_media_dimension(xml, "Height").unwrap_or(152.4);

    let orientation = if width_mm > height_mm {
        Orientation::Landscape
    } else {
        Orientation::Portrait
    };

    let mut label = Label {
        name: format!("{}.nlbl", file_stem),
        width_mm,
        height_mm,
        orientation,
        resolution: Resolution::Dpi203,
        margin_left_mm: 1.0,
        margin_top_mm: 1.0,
        margin_right_mm: 1.0,
        margin_bottom_mm: 1.0,
        elements: Vec::new(),
        next_element_id: 1,
    };

    // Extract Elements inside <DocumentDesigns> ... <Items> ... <Item Type="...">
    let mut rest = xml;
    while let Some(item_start) = rest.find("<Item Type=\"") {
        rest = &rest[item_start + 12..];
        let type_end = rest.find('"').unwrap_or(0);
        let item_type = &rest[..type_end];

        let item_body_end = rest.find("</Item>").unwrap_or(rest.len());
        let item_xml = &rest[..item_body_end];

        let name = extract_tag_value(item_xml, "Name")
            .unwrap_or_else(|| format!("Element_{}", label.next_element_id));

        // Geometry: X, Y in mm (raw units are 1/1000 mm)
        let x_mm = extract_geometry_coord(item_xml, "X").unwrap_or(5.0);
        let y_mm = extract_geometry_coord(item_xml, "Y").unwrap_or(5.0);

        label.next_element_id += 1;
        let id = label.next_element_id;

        if item_type.contains("Text") {
            let fixed = extract_tag_value(item_xml, "FixedContents")
                .or_else(|| extract_tag_value(item_xml, "StringValue"));
            let text = fixed
                .or_else(|| variables.get(&name).cloned())
                .unwrap_or_else(|| "Sample Text".to_string());

            let font_h = extract_tag_value(item_xml, "Height")
                .and_then(|h| h.parse::<f32>().ok())
                .unwrap_or(10.0);

            let font_size = if font_h > 0.0 { font_h * 1.3 } else { 12.0 };

            label.elements.push(Element {
                id,
                name,
                x: x_mm,
                y: y_mm,
                width: ((text.len() as f32) * (font_size * 0.4)).max(15.0).min(width_mm - x_mm),
                height: (font_size * 0.55).max(4.0),
                rotation: 0,
                data: ElementData::Text {
                    text,
                    font_size: font_size.clamp(6.0, 72.0),
                    bold: true,
                    italic: false,
                    color: [0, 0, 0, 255],
                },
            });
        } else if item_type.contains("Barcode") {
            let fixed = extract_tag_value(item_xml, "FixedContents")
                .or_else(|| extract_tag_value(item_xml, "StringValue"));
            let data = variables
                .get(&name)
                .cloned()
                .or(fixed)
                .unwrap_or_else(|| "123456789".to_string());

            let is_qr = item_xml.contains("QRBarcodeData") || item_xml.contains("QR");
            let is_c39 = item_xml.contains("Code39") || item_xml.contains("3of9");

            let b_type = if is_qr {
                BarcodeType::QrCode
            } else if is_c39 {
                BarcodeType::Code39
            } else {
                BarcodeType::Code128
            };

            let (w, h) = if is_qr {
                let size = (height_mm * 0.5).clamp(8.0, 40.0);
                (size, size)
            } else {
                (45.0, 18.0)
            };

            label.elements.push(Element {
                id,
                name,
                x: x_mm,
                y: y_mm,
                width: w,
                height: h,
                rotation: 0,
                data: ElementData::Barcode {
                    barcode_type: b_type,
                    data,
                    show_text: !is_qr,
                    module_width: 0.5,
                },
            });
        } else if item_type.contains("Graphic") || item_type.contains("Box") || item_type.contains("Shape") {
            let w = extract_geometry_coord(item_xml, "Width").unwrap_or(20.0);
            let h = extract_geometry_coord(item_xml, "Height").unwrap_or(10.0);

            label.elements.push(Element {
                id,
                name,
                x: x_mm,
                y: y_mm,
                width: w,
                height: h,
                rotation: 0,
                data: ElementData::Shape {
                    kind: ShapeKind::Rectangle,
                    stroke_width: 0.8,
                    stroke_color: [0, 0, 0, 255],
                    fill_color: None,
                    corner_radius: 0.0,
                },
            });
        }

        rest = &rest[item_body_end..];
    }

    Ok(label)
}

fn extract_tag_value(xml: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{}>", tag);
    let close_tag = format!("</{}>", tag);

    if let Some(start) = xml.find(&open_tag) {
        let content_start = start + open_tag.len();
        if let Some(end) = xml[content_start..].find(&close_tag) {
            let val = &xml[content_start..content_start + end];
            return Some(val.trim().to_string());
        }
    }
    None
}

fn extract_geometry_coord(xml: &str, coord_name: &str) -> Option<f32> {
    if let Some(geom_start) = xml.find("<Geometry") {
        let geom_slice = &xml[geom_start..];
        let geom_end = geom_slice.find("</Geometry>").unwrap_or(geom_slice.len());
        let geom_body = &geom_slice[..geom_end];

        if let Some(val_str) = extract_tag_value(geom_body, coord_name) {
            if let Ok(raw) = val_str.parse::<f32>() {
                return Some(raw / 1000.0); // 1000 units = 1 mm
            }
        }
    }
    None
}

fn extract_media_dimension(xml: &str, tag: &str) -> Option<f32> {
    if let Some(media_start) = xml.find("<Media") {
        let media_slice = &xml[media_start..];
        let media_end = media_slice.find("</Media>").unwrap_or(media_slice.len());
        let media_body = &media_slice[..media_end];

        if let Some(val_str) = extract_tag_value(media_body, tag) {
            if let Ok(raw) = val_str.parse::<f32>() {
                return Some(raw / 1000.0);
            }
        }
    }
    None
}
