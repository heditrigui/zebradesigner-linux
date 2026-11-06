use serde::{Deserialize, Serialize};

/// Text for a counter value, e.g. `SN-001042`.
pub fn counter_text(prefix: &str, value: i64, padding_digits: usize) -> String {
    format!("{}{:0width$}", prefix, value, width = padding_digits)
}

/// Current local time in a strftime format; an invalid format is shown as typed.
pub fn format_datetime(format_str: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    match write!(out, "{}", chrono::Local::now().format(format_str)) {
        Ok(()) => out,
        Err(_) => format_str.to_string(),
    }
}

/// Thermal printers are one-colour: dark colours print, light ones stay paper white.
pub fn prints_black(color: [u8; 4]) -> bool {
    let luminance = 0.299 * color[0] as f32 + 0.587 * color[1] as f32 + 0.114 * color[2] as f32;
    color[3] >= 128 && luminance < 128.0
}

/// Splits an element box into (length along the reading direction, length across it).
pub fn along_across<T: Copy>(rotation: i32, width: T, height: T) -> (T, T) {
    if rotation == 90 || rotation == 270 {
        (height, width)
    } else {
        (width, height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Mm,
    Inch,
    Dots,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Dpi203, // 8 dots/mm
    Dpi300, // 12 dots/mm
    Dpi600, // 24 dots/mm
}

impl Resolution {
    pub fn dpi(&self) -> u32 {
        match self {
            Resolution::Dpi203 => 203,
            Resolution::Dpi300 => 300,
            Resolution::Dpi600 => 600,
        }
    }

    pub fn dots_per_mm(&self) -> f32 {
        match self {
            Resolution::Dpi203 => 8.0,
            Resolution::Dpi300 => 12.0,
            Resolution::Dpi600 => 24.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarcodeType {
    Code128,
    Code39,
    Ean13,
    UpcA,
    QrCode,
    DataMatrix,
}

impl BarcodeType {
    pub fn name(&self) -> &'static str {
        match self {
            BarcodeType::Code128 => "Code 128",
            BarcodeType::Code39 => "Code 39",
            BarcodeType::Ean13 => "EAN-13",
            BarcodeType::UpcA => "UPC-A",
            BarcodeType::QrCode => "QR Code (2D)",
            BarcodeType::DataMatrix => "Data Matrix (2D)",
        }
    }

    pub fn is_2d(&self) -> bool {
        matches!(self, BarcodeType::QrCode | BarcodeType::DataMatrix)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Line,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ElementData {
    Text {
        text: String,
        font_size: f32,
        bold: bool,
        italic: bool,
        color: [u8; 4],
    },
    Barcode {
        barcode_type: BarcodeType,
        data: String,
        show_text: bool,
        module_width: f32,
    },
    Shape {
        kind: ShapeKind,
        stroke_width: f32,
        stroke_color: [u8; 4],
        fill_color: Option<[u8; 4]>,
        corner_radius: f32,
    },
    Counter {
        prefix: String,
        current_value: i64,
        step: i64,
        padding_digits: usize,
        font_size: f32,
        bold: bool,
    },
    DateTime {
        format_str: String,
        font_size: f32,
        bold: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: u64,
    pub name: String,
    pub x: f32, // in mm
    pub y: f32, // in mm
    pub width: f32, // in mm
    pub height: f32, // in mm
    pub rotation: i32, // 0, 90, 180, 270
    pub data: ElementData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub width_mm: f32,
    pub height_mm: f32,
    pub orientation: Orientation,
    pub resolution: Resolution,
    pub margin_left_mm: f32,
    pub margin_top_mm: f32,
    pub margin_right_mm: f32,
    pub margin_bottom_mm: f32,
    pub elements: Vec<Element>,
    pub next_element_id: u64,
}

impl Default for Label {
    fn default() -> Self {
        let mut label = Label {
            name: "Standard Shipping Label".to_string(),
            width_mm: 101.6, // 4 inches
            height_mm: 152.4, // 6 inches
            orientation: Orientation::Portrait,
            resolution: Resolution::Dpi203,
            margin_left_mm: 2.0,
            margin_top_mm: 2.0,
            margin_right_mm: 2.0,
            margin_bottom_mm: 2.0,
            elements: Vec::new(),
            next_element_id: 100,
        };

        label.populate_sample_label();
        label
    }
}

impl Label {
    pub fn new_empty(width_mm: f32, height_mm: f32, name: &str) -> Self {
        Label {
            name: name.to_string(),
            width_mm,
            height_mm,
            orientation: Orientation::Portrait,
            resolution: Resolution::Dpi203,
            margin_left_mm: 2.0,
            margin_top_mm: 2.0,
            margin_right_mm: 2.0,
            margin_bottom_mm: 2.0,
            elements: Vec::new(),
            next_element_id: 1,
        }
    }

    pub fn has_counters(&self) -> bool {
        self.elements.iter().any(|e| matches!(e.data, ElementData::Counter { .. }))
    }

    /// Moves every counter past the labels that were just printed.
    pub fn advance_counters(&mut self, copies: u32) {
        for el in &mut self.elements {
            if let ElementData::Counter { current_value, step, .. } = &mut el.data {
                *current_value += *step * copies.max(1) as i64;
            }
        }
    }

    pub fn populate_sample_label(&mut self) {
        self.elements.clear();

        // Header Border Box
        self.elements.push(Element {
            id: 1,
            name: "Header Box".to_string(),
            x: 4.0,
            y: 4.0,
            width: 93.6,
            height: 18.0,
            rotation: 0,
            data: ElementData::Shape {
                kind: ShapeKind::RoundedRectangle,
                stroke_width: 0.8,
                stroke_color: [20, 20, 20, 255],
                fill_color: Some([240, 243, 246, 255]),
                corner_radius: 2.0,
            },
        });

        // Header Title
        self.elements.push(Element {
            id: 2,
            name: "Shipper Title".to_string(),
            x: 7.0,
            y: 6.5,
            width: 60.0,
            height: 6.0,
            rotation: 0,
            data: ElementData::Text {
                text: "GLOBAL EXPRESS LOGISTICS".to_string(),
                font_size: 14.0,
                bold: true,
                italic: false,
                color: [20, 20, 20, 255],
            },
        });

        // Priority Subtitle
        self.elements.push(Element {
            id: 3,
            name: "Priority Badge".to_string(),
            x: 7.0,
            y: 13.5,
            width: 50.0,
            height: 5.0,
            rotation: 0,
            data: ElementData::Text {
                text: "PRIORITY AIR CARGO - TRACKED".to_string(),
                font_size: 10.0,
                bold: false,
                italic: false,
                color: [70, 70, 70, 255],
            },
        });

        // QR Code in header
        self.elements.push(Element {
            id: 4,
            name: "Routing QR".to_string(),
            x: 82.0,
            y: 5.5,
            width: 14.0,
            height: 14.0,
            rotation: 0,
            data: ElementData::Barcode {
                barcode_type: BarcodeType::QrCode,
                data: "EXP-US-892410-NX".to_string(),
                show_text: false,
                module_width: 1.0,
            },
        });

        // Recipient Section Box
        self.elements.push(Element {
            id: 5,
            name: "Recipient Frame".to_string(),
            x: 4.0,
            y: 25.0,
            width: 93.6,
            height: 32.0,
            rotation: 0,
            data: ElementData::Shape {
                kind: ShapeKind::Rectangle,
                stroke_width: 0.6,
                stroke_color: [40, 40, 40, 255],
                fill_color: None,
                corner_radius: 0.0,
            },
        });

        self.elements.push(Element {
            id: 6,
            name: "Deliver To Label".to_string(),
            x: 7.0,
            y: 27.0,
            width: 40.0,
            height: 4.0,
            rotation: 0,
            data: ElementData::Text {
                text: "SHIP TO:".to_string(),
                font_size: 9.0,
                bold: true,
                italic: false,
                color: [100, 100, 100, 255],
            },
        });

        self.elements.push(Element {
            id: 7,
            name: "Recipient Name".to_string(),
            x: 7.0,
            y: 32.0,
            width: 80.0,
            height: 6.0,
            rotation: 0,
            data: ElementData::Text {
                text: "ACME CORPORATION - TECH LABS".to_string(),
                font_size: 13.0,
                bold: true,
                italic: false,
                color: [10, 10, 10, 255],
            },
        });

        self.elements.push(Element {
            id: 8,
            name: "Recipient Address 1".to_string(),
            x: 7.0,
            y: 39.0,
            width: 80.0,
            height: 5.0,
            rotation: 0,
            data: ElementData::Text {
                text: "100 INNOVATION WAY, SUITE 400".to_string(),
                font_size: 11.0,
                bold: false,
                italic: false,
                color: [30, 30, 30, 255],
            },
        });

        self.elements.push(Element {
            id: 9,
            name: "Recipient City".to_string(),
            x: 7.0,
            y: 45.0,
            width: 80.0,
            height: 5.0,
            rotation: 0,
            data: ElementData::Text {
                text: "AUSTIN, TX 78701 USA".to_string(),
                font_size: 12.0,
                bold: true,
                italic: false,
                color: [10, 10, 10, 255],
            },
        });

        // Middle Divider
        self.elements.push(Element {
            id: 10,
            name: "Divider Line".to_string(),
            x: 4.0,
            y: 60.0,
            width: 93.6,
            height: 0.5,
            rotation: 0,
            data: ElementData::Shape {
                kind: ShapeKind::Line,
                stroke_width: 0.8,
                stroke_color: [30, 30, 30, 255],
                fill_color: None,
                corner_radius: 0.0,
            },
        });

        // Package Details
        self.elements.push(Element {
            id: 11,
            name: "Weight Label".to_string(),
            x: 6.0,
            y: 63.0,
            width: 40.0,
            height: 5.0,
            rotation: 0,
            data: ElementData::Text {
                text: "WEIGHT: 4.5 KG / 9.9 LBS".to_string(),
                font_size: 11.0,
                bold: true,
                italic: false,
                color: [20, 20, 20, 255],
            },
        });

        self.elements.push(Element {
            id: 12,
            name: "Zone Badge".to_string(),
            x: 68.0,
            y: 62.5,
            width: 28.0,
            height: 8.0,
            rotation: 0,
            data: ElementData::Shape {
                kind: ShapeKind::RoundedRectangle,
                stroke_width: 0.8,
                stroke_color: [0, 0, 0, 255],
                fill_color: Some([20, 20, 20, 255]),
                corner_radius: 1.5,
            },
        });

        self.elements.push(Element {
            id: 13,
            name: "Zone Text".to_string(),
            x: 73.0,
            y: 64.0,
            width: 20.0,
            height: 5.0,
            rotation: 0,
            data: ElementData::Text {
                text: "ZONE 2 - FAST".to_string(),
                font_size: 10.0,
                bold: true,
                italic: false,
                color: [255, 255, 255, 255],
            },
        });

        // Main Tracking Barcode (Code 128)
        self.elements.push(Element {
            id: 14,
            name: "Primary Tracking Barcode".to_string(),
            x: 8.0,
            y: 74.0,
            width: 85.6,
            height: 28.0,
            rotation: 0,
            data: ElementData::Barcode {
                barcode_type: BarcodeType::Code128,
                data: "420787019205590123".to_string(),
                show_text: true,
                module_width: 0.45,
            },
        });

        // Secondary Divider
        self.elements.push(Element {
            id: 15,
            name: "Bottom Divider".to_string(),
            x: 4.0,
            y: 106.0,
            width: 93.6,
            height: 0.5,
            rotation: 0,
            data: ElementData::Shape {
                kind: ShapeKind::Line,
                stroke_width: 0.6,
                stroke_color: [50, 50, 50, 255],
                fill_color: None,
                corner_radius: 0.0,
            },
        });

        // Item / SKU Barcode (Code 39)
        self.elements.push(Element {
            id: 16,
            name: "SKU Barcode".to_string(),
            x: 6.0,
            y: 111.0,
            width: 58.0,
            height: 20.0,
            rotation: 0,
            data: ElementData::Barcode {
                barcode_type: BarcodeType::Code39,
                data: "SKU-9042-X".to_string(),
                show_text: true,
                module_width: 0.4,
            },
        });

        // Dynamic Counter
        self.elements.push(Element {
            id: 17,
            name: "Serial Counter".to_string(),
            x: 68.0,
            y: 112.0,
            width: 28.0,
            height: 6.0,
            rotation: 0,
            data: ElementData::Counter {
                prefix: "SN-".to_string(),
                current_value: 1042,
                step: 1,
                padding_digits: 6,
                font_size: 11.0,
                bold: true,
            },
        });

        // Date Timestamp
        self.elements.push(Element {
            id: 18,
            name: "Print Timestamp".to_string(),
            x: 68.0,
            y: 122.0,
            width: 28.0,
            height: 5.0,
            rotation: 0,
            data: ElementData::DateTime {
                format_str: "%Y-%m-%d %H:%M".to_string(),
                font_size: 9.0,
                bold: false,
            },
        });

        // Footer note
        self.elements.push(Element {
            id: 19,
            name: "Footer Note".to_string(),
            x: 6.0,
            y: 140.0,
            width: 88.0,
            height: 5.0,
            rotation: 0,
            data: ElementData::Text {
                text: "ZEBRA-COMPLIANT ZPL II EMULATION - PRINTABLE AT 203/300 DPI".to_string(),
                font_size: 8.0,
                bold: false,
                italic: false,
                color: [90, 90, 90, 255],
            },
        });
    }
}
