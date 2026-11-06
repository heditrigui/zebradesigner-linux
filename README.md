# Label Designer for Linux

A desktop designer for thermal barcode labels, written in Rust with egui. It prints to Zebra and Eltron printers using ZPL II or EPL2.

## Features

- Design labels with text, Code 128, Code 39 and QR barcodes, shapes, serial counters and date/time fields
- Preview labels at their printed size, at 203, 300 or 600 DPI
- Print through CUPS, directly to `/dev/usb/lp*`, or over the network (raw TCP, port 9100 by default)
- Import simple NiceLabel / ZebraDesigner `.nlbl` templates (requires `7z`)

## Build and run

Debian/Ubuntu may need the usual X11/Wayland and OpenGL development packages.

```bash
cargo run --release
```

To open a template on startup:

```bash
cargo run --release -- --import /path/to/label.nlbl
```

## Print behaviour

- Bar width and QR size are chosen to fit the element's box.
- Colours print as black or paper white, depending on how dark they are. Light text prints reversed.
- Bold is printed by striking text twice. Italic is not available.
- EPL2 prints rounded rectangles and ellipses as plain boxes.
- QR codes always print unrotated.
- Labels with a counter are sent as one format per copy, and the counter advances after a successful send.

A successful send means CUPS or the printer connection accepted the job. It does not confirm that a label came out.

## Known gaps

- Undo, redo, cut, copy and paste are not implemented.
- Save and Open use `label_project.json` in the current folder.
- The label orientation setting has no effect yet.

## Tests

```bash
cargo test
```

The tests check the generated printer commands and deliver a job to a local TCP socket. They do not use a physical printer.
