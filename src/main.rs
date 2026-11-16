#![windows_subsystem = "windows"]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.as_slice() {
        [] => reel::tray::app::main(reel::tray::app::Launch::Quiet),
        [command] if command == "show" => reel::tray::app::main(reel::tray::app::Launch::Greet),
        [command] if command == "quit" => reel::tray::control::quit(),
        [command] if command == "cleanup" => reel::tray::control::cleanup(),
        [command, window] if command == "record" => match window.parse::<isize>() {
            Ok(window) => reel::record::run::main(window),
            Err(_) => 2,
        },
        _ => 2,
    };
    std::process::exit(code);
}
