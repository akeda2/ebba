fn main() {
    if let Err(error) = ebba::app::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
