use clap::Parser;
use ukyo_clicker::hid::ReportHandler;
use ukyo_clicker::protocol::parse_input;

/// Right Joy-Con as a presentation clicker.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Print raw and decoded HID reports; send no keys.
    #[arg(long)]
    dump: bool,
}

fn dump(report_id: u32, bytes: &[u8]) {
    // borrow each byte, format it as two hex digits, and gather the results into a Vec<String>
    // `{b:02x}`: hex (`x`), zero-padded (`0`) to a minimum width of 2, e.g. 5 -> "05", 255 -> "ff".
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
    println!(
        // `{report_id:#04x}`: hex with the `#` alternate flag, which adds the `0x` prefix.
        // The width 4 counts that prefix, so the value gets 2 digits: 1 -> "0x01".
        // `{:>2}`: right-align (`>`) within a minimum width of 2 (padded with spaces).
        "id={report_id:#04x} len={:>2}: {}",
        bytes.len(),
        hex.join(" ")
    );
    match parse_input(bytes) {
        Ok(report) => println!("  -> {report:?}"),
        Err(e) => println!("  -> {e:?}"),
    }
}

fn ignore(_report_id: u32, _bytes: &[u8]) {}

fn main() {
    let args = Args::parse();
    println!("{}", ukyo_clicker::banner());

    let handler: ReportHandler = if args.dump { dump } else { ignore };
    if let Err(code) = ukyo_clicker::hid::run(handler) {
        // `{code:#x}`: hex with the `#` alternate flag (`0x` prefix), no width/padding.
        eprintln!("IOHIDManagerOpen failed: {code:#x}");
        std::process::exit(1);
    }
}
