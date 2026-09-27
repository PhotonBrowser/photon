fn main() {
    if let Err(error) = photon_cli::run() {
        use owo_colors::{OwoColorize, Stream};
        eprintln!(
            "{} {error}",
            "error:".if_supports_color(Stream::Stderr, |text| format!("{}", text.red().bold()))
        );
        std::process::exit(1);
    }
}
