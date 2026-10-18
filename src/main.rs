fn main() {
    println!("{}", ukyo_clicker::banner());
    if let Err(code) = ukyo_clicker::hid::run() {
        eprintln!("IOHIDManagerOpen failed: {code:#x}");
        std::process::exit(1);
    }
}
