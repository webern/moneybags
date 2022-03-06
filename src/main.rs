use clap::Parser;
use moneybags::Moneybags;
use std::io::stdout;

fn main() -> ! {
    let moneybags = Moneybags::parse();
    match moneybags.run(stdout()) {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1)
        }
    }
}
