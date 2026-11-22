use prism_app::{args, logging, preview};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = match args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("error: {error}\n\n{}", args::usage());
            std::process::exit(2);
        }
    };
    if args.help {
        println!("{}", args::usage());
        return Ok(());
    }

    // Slint reads this before its backend is initialized. The preview main is
    // single-threaded at this point, so setting it here is safe and makes
    // screenshot output reproducible across compositors.
    if let Some(scale) = args.scale {
        unsafe {
            std::env::set_var("SLINT_SCALE_FACTOR", scale.to_string());
        }
    }

    let _log_guard = logging::init(args.verbose);
    preview::run(args)
}
