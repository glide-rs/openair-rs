use std::fs::File;
use std::io::BufReader;

use openair::parse;

macro_rules! fail {
    ($msg:expr) => {{
        println!("Error: {}", $msg);
        std::process::exit(1);
    }};
}

fn main() -> std::io::Result<()> {
    env_logger::init();

    println!("Starting parser...");

    // Open file
    let filename = match std::env::args().nth(1) {
        Some(f) => f,
        None => fail!("Missing filename parameter"),
    };
    let file = File::open(filename)?;
    let mut reader = BufReader::new(file);

    // Process airspaces
    let airspaces = parse(&mut reader).unwrap_or_else(|e| fail!(e));
    println!("Airspaces:");
    for airspace in airspaces {
        println!("- {airspace}");
    }
    println!("Done.");

    Ok(())
}
