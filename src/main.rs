#![windows_subsystem = "windows"]

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.as_slice() {
        [] => reel::tray::app::main(),
        [command, window, output] if command == "record" => match window.parse::<isize>() {
            Ok(window) => reel::record::run::main(window, Path::new(output)),
            Err(_) => 2,
        },
        _ => 2,
    };
    std::process::exit(code);
}
