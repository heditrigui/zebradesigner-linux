use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

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
                    status: "Ready".to_string(),
                });
            }
        }
    }

    if printers.is_empty() {
        for (queue, uri) in &cups_queues {
            if uri.starts_with("usb://") {
                printers.push(UsbPrinter {
                    name: format!("Zebra {}", queue),
                    manufacturer: "Zebra Technologies".to_string(),
                    model: queue.clone(),
                    serial: "".to_string(),
                    vendor_id: "0a5f".to_string(),
                    product_id: "000a".to_string(),
                    device_node: "/dev/usb/lp0".to_string(),
                    cups_queue: Some(queue.clone()),
                    is_zebra: true,
                    status: "Ready".to_string(),
                });
            }
        }
    }

    if printers.is_empty() && Path::new("/dev/usb/lp0").exists() {
        let default_cups = cups_queues.first().map(|(q, _)| q.clone());
        printers.push(UsbPrinter {
            name: "Zebra Technologies TLP2844".to_string(),
            manufacturer: "Zebra Technologies".to_string(),
            model: "TLP2844".to_string(),
            serial: "41A063801083".to_string(),
            vendor_id: "0a5f".to_string(),
            product_id: "000a".to_string(),
            device_node: "/dev/usb/lp0".to_string(),
            cups_queue: default_cups.or_else(|| Some("TLP2844".to_string())),
            is_zebra: true,
            status: "Ready".to_string(),
        });
    }

    printers
}

pub fn print_to_usb(
    printer: &UsbPrinter,
    label: &crate::model::Label,
    copies: u32,
    lang_pref: PrinterLanguage,
) -> Result<String, String> {
    let use_epl = match lang_pref {
        PrinterLanguage::Epl2 => true,
        PrinterLanguage::Zpl2 => false,
        PrinterLanguage::Auto => printer.supports_epl(),
    };

    let print_data = if use_epl {
        crate::epl::generate_epl(label, copies)
    } else {
        crate::zpl::generate_zpl(label)
    };

    let protocol_name = if use_epl { "EPL2" } else { "ZPL II" };

    let tmp_path = std::env::temp_dir().join(format!("zebra_label_{}.prn", std::process::id()));
    if let Err(e) = fs::write(&tmp_path, &print_data) {
        return Err(format!("Failed to write temporary print file: {}", e));
    }

    let _ = fs::write("last_usb_print.prn", &print_data);

    // Method A: If CUPS queue is available, print raw
    if let Some(ref queue) = printer.cups_queue {
        let output = Command::new("lp")
            .arg("-d")
            .arg(queue)
            .arg("-n")
            .arg(copies.max(1).to_string())
            .arg("-o")
            .arg("raw")
            .arg(&tmp_path)
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let _ = fs::remove_file(&tmp_path);
                return Ok(format!(
                    "Print job sent to {} via CUPS ({} protocol, {} copies): {}",
                    queue, protocol_name, copies, stdout
                ));
            }
        }

        let lpr_out = Command::new("lpr")
            .arg("-P")
            .arg(queue)
            .arg(format!("-#{}", copies.max(1)))
            .arg("-l")
            .arg(&tmp_path)
            .output();

        if let Ok(out) = lpr_out {
            if out.status.success() {
                let _ = fs::remove_file(&tmp_path);
                return Ok(format!(
                    "Print job sent to {} via lpr ({} protocol, {} copies)",
                    queue, protocol_name, copies
                ));
            }
        }
    }

    // Method B: Direct character device write
    match fs::OpenOptions::new().write(true).open(&printer.device_node) {
        Ok(mut file) => {
            let mut err = None;
            for _ in 0..copies.max(1) {
                if let Err(e) = file.write_all(print_data.as_bytes()) {
                    err = Some(e);
                    break;
                }
            }
            let _ = fs::remove_file(&tmp_path);
            if let Some(e) = err {
                Err(format!("Error writing to {}: {}", printer.device_node, e))
            } else {
                Ok(format!(
                    "Direct print ({}): sent {} copies to {}",
                    protocol_name, copies, printer.device_node
                ))
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            Err(format!(
                "Unable to send to {}: {}. Add user to 'lp' group or configure CUPS queue.",
                printer.device_node, e
            ))
        }
    }
}
