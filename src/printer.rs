use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::barcode::{bar_dots, generate_code128, generate_code39, generate_qr_matrix, is_code39_char, qr_scale};
use crate::model::{along_across, BarcodeType, ElementData, Label, ShapeKind};

const NETWORK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrinterLanguage {
    Auto,
    Epl2,
    Zpl2,
}

impl PrinterLanguage {
    pub fn name(&self) -> &'static str {
        match self {
            PrinterLanguage::Auto => "Auto-Detect",
            PrinterLanguage::Epl2 => "EPL2 (Eltron)",
            PrinterLanguage::Zpl2 => "ZPL II (Zebra)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsbPrinter {
    pub name: String,
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub vendor_id: String,
    pub product_id: String,
    pub device_node: String,
    pub cups_queue: Option<String>,
    pub is_zebra: bool,
    pub status: String,
}

impl UsbPrinter {
    pub fn display_name(&self) -> String {
        let lang = if self.supports_epl() { "EPL2" } else { "ZPL II" };
        if let Some(ref q) = self.cups_queue {
            format!("{} {} [{}] ({})", self.manufacturer, self.model, lang, q)
        } else if !self.model.is_empty() {
            format!("{} {} [{}] ({})", self.manufacturer, self.model, lang, self.device_node)
        } else {
            format!("USB Printer ({})", self.device_node)
        }
    }

    pub fn supports_epl(&self) -> bool {
        let lower_name = self.name.to_lowercase();
        let lower_model = self.model.to_lowercase();
        let lower_queue = self.cups_queue.as_deref().unwrap_or("").to_lowercase();
        lower_name.contains("2844")
            || lower_name.contains("2824")
            || lower_name.contains("2742")
            || lower_name.contains("epl")
            || lower_name.contains("eltron")
            || lower_model.contains("2844")
            || lower_model.contains("2824")
            || lower_model.contains("tlp")
            || lower_model.contains("lp2")
            || lower_queue.contains("2844")
            || lower_queue.contains("epl")
    }
}

pub fn detect_usb_printers() -> Vec<UsbPrinter> {
    let mut printers = Vec::new();

    // 1. Check CUPS print queues via `lpstat -v`
    let mut cups_queues: Vec<(String, String)> = Vec::new();
    if let Ok(output) = Command::new("lpstat").arg("-v").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("device for ") {
                    if let Some(colon_idx) = rest.find(':') {
                        let queue = rest[..colon_idx].trim().to_string();
                        let uri = rest[colon_idx + 1..].trim().to_string();
                        cups_queues.push((queue, uri));
                    }
                }
            }
        }
    }

    // 2. Scan /sys/class/usbmisc/
    if let Ok(entries) = fs::read_dir("/sys/class/usbmisc") {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with("lp") {
                let dev_path = format!("/dev/usb/{}", filename);
                let fallback_dev = format!("/dev/{}", filename);
                let actual_dev = if Path::new(&dev_path).exists() {
                    dev_path
                } else if Path::new(&fallback_dev).exists() {
                    fallback_dev
                } else {
                    format!("/dev/usb/{}", filename)
                };

                let base_path = entry.path();
                let device_path = base_path.join("device");

                let mut manufacturer = String::new();
                let mut product = String::new();
                let mut serial = String::new();
                let mut vendor_id = String::new();
                let mut product_id = String::new();

                let parent_path = device_path.join("..");
                if let Ok(mfg) = fs::read_to_string(parent_path.join("manufacturer")) {
                    manufacturer = mfg.trim().trim_end_matches(';').trim().to_string();
                }
                if let Ok(prod) = fs::read_to_string(parent_path.join("product")) {
                    product = prod.trim().to_string();
                }
                if let Ok(ser) = fs::read_to_string(parent_path.join("serial")) {
                    serial = ser.trim().to_string();
                }
                if let Ok(v_id) = fs::read_to_string(parent_path.join("idVendor")) {
                    vendor_id = v_id.trim().to_string();
                }
                if let Ok(p_id) = fs::read_to_string(parent_path.join("idProduct")) {
                    product_id = p_id.trim().to_string();
                }

                if manufacturer.is_empty() || product.is_empty() {
                    if let Ok(ieee) = fs::read_to_string(device_path.join("ieee1284_id")) {
                        for part in ieee.split(';') {
                            if let Some(val) = part.strip_prefix("MFG:") {
                                if manufacturer.is_empty() {
                                    manufacturer = val.trim().to_string();
                                }
                            } else if let Some(val) = part.strip_prefix("MDL:") {
                                if product.is_empty() {
                                    product = val.trim().to_string();
                                }
                            }
                        }
                    }
                }

                let is_zebra = vendor_id == "0a5f"
                    || manufacturer.to_lowercase().contains("zebra")
                    || product.to_lowercase().contains("tlp")
                    || product.to_lowercase().contains("zd");

                if manufacturer.is_empty() {
                    manufacturer = if is_zebra { "Zebra Technologies".to_string() } else { "Generic".to_string() };
                }
                if product.is_empty() {
                    product = if is_zebra { "Label Printer".to_string() } else { "USB Printer".to_string() };
                }

                let mut matched_queue = None;
                for (queue, uri) in &cups_queues {
                    let uri_lower = uri.to_lowercase();
                    if (!serial.is_empty() && uri.contains(&serial))
                        || (!product.is_empty() && uri_lower.contains(&product.to_lowercase()))
                        || (is_zebra && uri_lower.contains("zebra"))
                    {
                        matched_queue = Some(queue.clone());
                        break;
                    }
                }

                let printer_name = format!("{} {}", manufacturer, product).trim().to_string();

                printers.push(UsbPrinter {
                    name: printer_name,
                    manufacturer,
                    model: product,
                    serial,
                    vendor_id,
                    product_id,
                    device_node: actual_dev,
                    cups_queue: matched_queue,
                    is_zebra,
                    status: "Detected".to_string(),
                });
            }
        }
    }

    // Under CUPS the usblp driver is often unbound, so the queue address is the only record
    if printers.is_empty() {
        for (queue, uri) in &cups_queues {
            if let Some((manufacturer, model, serial)) = parse_usb_uri(uri) {
                let is_zebra = manufacturer.to_lowercase().contains("zebra");
                printers.push(UsbPrinter {
                    name: format!("{} {}", manufacturer, model),
                    manufacturer,
                    model,
                    serial,
                    vendor_id: String::new(),
                    product_id: String::new(),
                    device_node: "/dev/usb/lp0".to_string(),
                    cups_queue: Some(queue.clone()),
                    is_zebra,
                    status: "Detected".to_string(),
                });
            }
        }
    }

    if printers.is_empty() && Path::new("/dev/usb/lp0").exists() {
        printers.push(UsbPrinter {
            name: "USB Printer".to_string(),
            manufacturer: "Generic".to_string(),
            model: "USB Printer".to_string(),
            serial: String::new(),
            vendor_id: String::new(),
            product_id: String::new(),
            device_node: "/dev/usb/lp0".to_string(),
            cups_queue: None,
            is_zebra: false,
            status: "Detected".to_string(),
        });
    }

    printers.sort_by_key(|p| !p.is_zebra);
    printers
}

/// Reads manufacturer, model and serial from a CUPS address like
/// `usb://Zebra/TLP2844?serial=ABC123`.
fn parse_usb_uri(uri: &str) -> Option<(String, String, String)> {
    let rest = uri.strip_prefix("usb://")?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (manufacturer, model) = path.split_once('/')?;
    let serial = query
        .split('&')
        .find_map(|part| part.strip_prefix("serial="))
        .unwrap_or("");
    let decode = |text: &str| text.replace("%20", " ");
    Some((decode(manufacturer), decode(model), serial.to_string()))
}

/// Whether a job uses EPL2. Network printers have no model to detect, so Auto means ZPL.
pub fn resolve_use_epl(lang_pref: PrinterLanguage, printer: Option<&UsbPrinter>) -> bool {
    match lang_pref {
        PrinterLanguage::Epl2 => true,
        PrinterLanguage::Zpl2 => false,
        PrinterLanguage::Auto => printer.map_or(false, UsbPrinter::supports_epl),
    }
}

/// Refuses labels that would print wrong and returns notes about anything the
/// printer can only approximate.
pub fn check_label(label: &Label, use_epl: bool) -> Result<Vec<String>, String> {
    let dpmm = label.resolution.dots_per_mm();
    let mut notes = Vec::new();

    for el in &label.elements {
        let w = (el.width * dpmm).round().max(1.0) as u32;
        let h = (el.height * dpmm).round().max(1.0) as u32;
        match &el.data {
            ElementData::Barcode { barcode_type, data, .. } => {
                if data.is_empty() {
                    return Err(format!("\"{}\" has no barcode data.", el.name));
                }
                let (along, _) = along_across(el.rotation, w, h);
                match barcode_type {
                    BarcodeType::Code128 => {
                        if !data.bytes().all(|b| (32..=126).contains(&b)) {
                            return Err(format!("\"{}\": Code 128 only accepts plain ASCII text.", el.name));
                        }
                        note_if_too_wide(&mut notes, &el.name, generate_code128(data).modules.len(), along);
                    }
                    BarcodeType::Code39 => {
                        if !data.trim_matches('*').chars().all(is_code39_char) {
                            return Err(format!("\"{}\": Code 39 only accepts A-Z, 0-9 and - . $ / + % space.", el.name));
                        }
                        note_if_too_wide(&mut notes, &el.name, generate_code39(data).modules.len(), along);
                    }
                    BarcodeType::QrCode => {
                        if generate_qr_matrix(data).is_none() {
                            return Err(format!("\"{}\": too much data for a QR code.", el.name));
                        }
                        if qr_scale(data, w.min(h)) < 2 {
                            notes.push(format!("\"{}\": QR code may be too small to scan.", el.name));
                        }
                        if el.rotation != 0 {
                            notes.push(format!("\"{}\": QR codes print unrotated.", el.name));
                        }
                    }
                    other => {
                        return Err(format!("\"{}\": {} printing is not supported yet.", el.name, other.name()));
                    }
                }
            }
            ElementData::Shape { kind: ShapeKind::RoundedRectangle | ShapeKind::Ellipse, .. } if use_epl => {
                notes.push(format!("\"{}\": EPL2 prints this shape as a plain box.", el.name));
            }
            ElementData::Text { italic: true, .. } => {
                notes.push(format!("\"{}\": italic text prints upright.", el.name));
            }
            _ => {}
        }
    }

    Ok(notes)
}

fn note_if_too_wide(notes: &mut Vec<String>, name: &str, module_count: usize, along_dots: u32) {
    if module_count as u32 * bar_dots(module_count, along_dots) > along_dots {
        notes.push(format!("\"{}\": barcode is wider than its box.", name));
    }
}

fn job_data(label: &Label, copies: u32, use_epl: bool) -> String {
    if use_epl {
        crate::epl::generate_epl(label, copies)
    } else {
        crate::zpl::generate_zpl(label, copies)
    }
}

fn protocol_name(use_epl: bool) -> &'static str {
    if use_epl {
        "EPL2"
    } else {
        "ZPL II"
    }
}

fn with_notes(message: String, notes: &[String]) -> String {
    if notes.is_empty() {
        message
    } else {
        format!("{} Note: {}", message, notes.join(" "))
    }
}

pub fn print_to_usb(
    printer: &UsbPrinter,
    label: &mut Label,
    copies: u32,
    lang_pref: PrinterLanguage,
) -> Result<String, String> {
    let copies = copies.max(1);
    let use_epl = resolve_use_epl(lang_pref, Some(printer));
    let notes = check_label(label, use_epl)?;
    let print_data = job_data(label, copies, use_epl);
    let protocol = protocol_name(use_epl);

    let mut failures = Vec::new();

    // Method A: If CUPS queue is available, print raw
    if let Some(ref queue) = printer.cups_queue {
        match send_to_cups(queue, &print_data) {
            Ok(request) => {
                label.advance_counters(copies);
                let message = format!("Print job sent to {} via CUPS ({}, {} copies): {}", queue, protocol, copies, request);
                return Ok(with_notes(message, &notes));
            }
            Err(e) => failures.push(e),
        }
    }

    // Method B: Direct character device write
    match fs::OpenOptions::new()
        .write(true)
        .open(&printer.device_node)
        .and_then(|mut file| file.write_all(print_data.as_bytes()))
    {
        Ok(()) => {
            label.advance_counters(copies);
            let message = format!("Direct print ({}): sent {} copies to {}", protocol, copies, printer.device_node);
            Ok(with_notes(message, &notes))
        }
        Err(e) => {
            failures.push(format!("{}: {}", printer.device_node, e));
            Err(failures.join("; "))
        }
    }
}

fn send_to_cups(queue: &str, print_data: &str) -> Result<String, String> {
    let tmp_path = std::env::temp_dir().join(format!("zebra_label_{}.prn", std::process::id()));
    fs::write(&tmp_path, print_data).map_err(|e| format!("Failed to write temporary print file: {}", e))?;

    let output = Command::new("lp")
        .arg("-d")
        .arg(queue)
        .arg("-o")
        .arg("raw")
        .arg(&tmp_path)
        .output();
    let _ = fs::remove_file(&tmp_path);

    match output {
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).trim().to_string()),
        Ok(out) => Err(format!("lp: {}", String::from_utf8_lossy(&out.stderr).trim())),
        Err(e) => Err(format!("lp: {}", e)),
    }
}

pub fn print_to_network(
    host: &str,
    port: u16,
    label: &mut Label,
    copies: u32,
    lang_pref: PrinterLanguage,
) -> Result<String, String> {
    let copies = copies.max(1);
    let use_epl = resolve_use_epl(lang_pref, None);
    let notes = check_label(label, use_epl)?;
    let print_data = job_data(label, copies, use_epl);

    let address = (host.trim(), port)
        .to_socket_addrs()
        .map_err(|e| format!("Cannot resolve {}: {}", host, e))?
        .next()
        .ok_or_else(|| format!("Cannot resolve {}", host))?;

    let mut stream = TcpStream::connect_timeout(&address, NETWORK_TIMEOUT)
        .map_err(|e| format!("Cannot connect to {}: {}", address, e))?;
    stream
        .set_write_timeout(Some(NETWORK_TIMEOUT))
        .map_err(|e| format!("Cannot configure connection to {}: {}", address, e))?;
    stream
        .write_all(print_data.as_bytes())
        .and_then(|_| stream.flush())
        .map_err(|e| format!("Error sending to {}: {}", address, e))?;

    label.advance_counters(copies);
    let message = format!("Print job sent to {} ({}, {} copies)", address, protocol_name(use_epl), copies);
    Ok(with_notes(message, &notes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Element;
    use std::io::Read;
    use std::net::TcpListener;

    fn counter_label() -> Label {
        let mut label = Label::new_empty(50.0, 25.0, "test");
        label.elements.push(Element {
            id: 1,
            name: "serial".to_string(),
            x: 1.0,
            y: 1.0,
            width: 40.0,
            height: 5.0,
            rotation: 0,
            data: ElementData::Counter {
                prefix: "SN-".to_string(),
                current_value: 10,
                step: 1,
                padding_digits: 3,
                font_size: 10.0,
                bold: false,
            },
        });
        label
    }

    fn barcode_label(barcode_type: BarcodeType, data: &str) -> Label {
        let mut label = Label::new_empty(50.0, 25.0, "test");
        label.elements.push(Element {
            id: 1,
            name: "code".to_string(),
            x: 1.0,
            y: 1.0,
            width: 40.0,
            height: 10.0,
            rotation: 0,
            data: ElementData::Barcode { barcode_type, data: data.to_string(), show_text: false, module_width: 0.4 },
        });
        label
    }

    #[test]
    fn network_print_delivers_the_job_and_advances_counters() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let receiver = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut received = String::new();
            stream.read_to_string(&mut received).unwrap();
            received
        });

        let mut label = counter_label();
        print_to_network("127.0.0.1", port, &mut label, 2, PrinterLanguage::Zpl2).unwrap();

        let received = receiver.join().unwrap();
        assert!(received.contains("SN-010") && received.contains("SN-011"), "{received}");
        assert!(matches!(label.elements[0].data, ElementData::Counter { current_value: 12, .. }));
    }

    #[test]
    fn failed_network_print_is_an_error_and_keeps_counters() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut label = counter_label();
        assert!(print_to_network("127.0.0.1", port, &mut label, 2, PrinterLanguage::Zpl2).is_err());
        assert!(matches!(label.elements[0].data, ElementData::Counter { current_value: 10, .. }));
    }

    #[test]
    fn reads_printer_details_from_cups_address() {
        let (manufacturer, model, serial) = parse_usb_uri("usb://Zebra/TLP2844?serial=ABC123").unwrap();
        assert_eq!((manufacturer.as_str(), model.as_str(), serial.as_str()), ("Zebra", "TLP2844", "ABC123"));
        assert!(parse_usb_uri("socket://192.0.2.1:9100").is_none());
    }

    #[test]
    fn auto_language_uses_epl_only_for_known_models() {
        assert!(!resolve_use_epl(PrinterLanguage::Auto, None));
        assert!(resolve_use_epl(PrinterLanguage::Epl2, None));
    }

    #[test]
    fn unsupported_or_invalid_barcodes_are_refused() {
        assert!(check_label(&barcode_label(BarcodeType::Ean13, "123"), false).is_err());
        assert!(check_label(&barcode_label(BarcodeType::Code39, "a_b"), false).is_err());
        assert!(check_label(&barcode_label(BarcodeType::Code128, "caf\u{e9}"), false).is_err());
        assert!(check_label(&barcode_label(BarcodeType::Code128, ""), false).is_err());
        assert!(check_label(&barcode_label(BarcodeType::Code128, "OK-1"), false).is_ok());
    }
}
