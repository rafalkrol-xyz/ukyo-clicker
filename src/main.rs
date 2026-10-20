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
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
    println!(
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
        eprintln!("IOHIDManagerOpen failed: {code:#x}");
        std::process::exit(1);
    }
}
