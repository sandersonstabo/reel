#![windows_subsystem = "windows"]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.as_slice() {
        [] => reel::tray::app::main(),
        [command, window] if command == "record" => match window.parse::<isize>() {
            Ok(window) => reel::record::run::main(window),
            Err(_) => 2,
        },
        _ => 2,
    };
    std::process::exit(code);
}
