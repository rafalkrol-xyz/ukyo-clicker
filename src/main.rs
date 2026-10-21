use clap::Parser;
use ukyo_clicker::config::Config;
use ukyo_clicker::hid::ReportHandler;
use ukyo_clicker::keys::{CGEventSender, DryRun, KeySender, accessibility_trusted};
use ukyo_clicker::mapper::{Action, Mapper};
use ukyo_clicker::protocol::{InputReport, parse_input};

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

fn main() {
    let args = Args::parse();
    println!("{}", ukyo_clicker::banner());

    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let mut mapper = Mapper::new(config.next, config.prev);
    let dump_enabled = args.dump;
    if !dump_enabled && !accessibility_trusted(true) {
        eprintln!(
            "Accessibility permission missing. Enable your terminal in\n\
             System Settings > Privacy & Security > Accessibility, then run again."
        );
        std::process::exit(1);
    }
    let mut sender: Box<dyn KeySender> = if dump_enabled {
        Box::new(DryRun)
    } else {
        Box::new(CGEventSender)
    };

    let handler: ReportHandler = Box::new(move |report_id, bytes| {
        if dump_enabled {
            dump(report_id, bytes);
        }
        let Ok(InputReport::Full(state)) = parse_input(bytes) else {
            return;
        };
        for action in mapper.update(state.buttons) {
            match action {
                Action::Next => println!("→ NEXT"),
                Action::Prev => println!("← PREV"),
            }
            sender.send(action);
        }
    });

    if let Err(code) = ukyo_clicker::hid::run(handler) {
        eprintln!("IOHIDManagerOpen failed: {code:#x}");
        std::process::exit(1);
    }
}
