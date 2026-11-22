use std::path::PathBuf;

use prism_platform::PreviewScenario;

#[derive(Clone, Debug)]
pub struct PreviewArgs {
    pub scenario: PreviewScenario,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub screenshot: Option<PathBuf>,
    pub open_cc: bool,
    pub open_launcher: bool,
    pub preview_app: Option<String>,
    pub confirm: bool,
    pub toast: Option<String>,
    pub reduced_motion: bool,
    pub scale: Option<f32>,
    pub config: Option<PathBuf>,
    pub verbose: bool,
    pub twelve_hour: bool,
    pub no_date: bool,
    pub help: bool,
}

impl Default for PreviewArgs {
    fn default() -> Self {
        Self {
            scenario: PreviewScenario::Laptop,
            width: 1600,
            height: 1000,
            fullscreen: false,
            screenshot: None,
            open_cc: false,
            open_launcher: false,
            preview_app: None,
            confirm: false,
            toast: None,
            reduced_motion: false,
            scale: None,
            config: None,
            verbose: false,
            twelve_hour: false,
            no_date: false,
            help: false,
        }
    }
}

pub fn usage() -> &'static str {
    "Prism Shell — Linux development preview

USAGE:
  prism-preview [OPTIONS]

OPTIONS:
  --scenario <laptop|desktop|minimal|faults|many>   fixture (default: laptop)
  --width <px>                                      logical window width (default: 1600)
  --height <px>                                     logical window height (default: 1000)
  --fullscreen                                      ask the window manager for fullscreen
  --screenshot <path>                               render once, save a PNG, and exit
  --open-cc                                         open Control Center on start
  --open-launcher                                   open the application launcher
  --preview <app-id>                                open window previews for an app
  --confirm                                         open the power confirmation sheet
  --toast <message>                                 show a sample toast
  --reduced-motion                                  disable large transforms
  --scale <factor>                                  force a logical scale factor (0.5–4.0)
  --12h                                             use a 12-hour clock
  --no-date                                         hide the date under the clock
  --config <path>                                   load a specific config JSON
  --verbose                                         debug logging
  -h, --help                                        print this help

This executable uses explicit mock platform services. It does not change any
Windows or Linux system state. It is not the production shell."
}

pub fn parse<I>(args: I) -> Result<PreviewArgs, String>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = PreviewArgs::default();
    let mut iter = args.into_iter().peekable();

    while let Some(argument) = iter.next() {
        let (flag, inline) = match argument.split_once('=') {
            Some((flag, value)) => (flag.to_owned(), Some(value.to_owned())),
            None => (argument, None),
        };
        let mut value = || -> Result<String, String> {
            inline
                .clone()
                .or_else(|| iter.next())
                .ok_or_else(|| format!("{flag} requires a value"))
        };

        match flag.as_str() {
            "-h" | "--help" => parsed.help = true,
            "--scenario" => {
                let id = value()?;
                parsed.scenario = PreviewScenario::from_id(&id)
                    .ok_or_else(|| format!("unknown scenario '{id}'"))?;
            }
            "--width" => {
                let width: u32 = value()?
                    .parse()
                    .map_err(|_| "--width must be a positive integer".to_owned())?;
                if !(640..=7680).contains(&width) {
                    return Err("--width must be between 640 and 7680".to_owned());
                }
                parsed.width = width;
            }
            "--height" => {
                let height: u32 = value()?
                    .parse()
                    .map_err(|_| "--height must be a positive integer".to_owned())?;
                if !(480..=4320).contains(&height) {
                    return Err("--height must be between 480 and 4320".to_owned());
                }
                parsed.height = height;
            }
            "--screenshot" => parsed.screenshot = Some(PathBuf::from(value()?)),
            "--preview" => parsed.preview_app = Some(value()?),
            "--toast" => parsed.toast = Some(value()?),
            "--config" => parsed.config = Some(PathBuf::from(value()?)),
            "--fullscreen" => parsed.fullscreen = true,
            "--open-cc" => parsed.open_cc = true,
            "--open-launcher" => parsed.open_launcher = true,
            "--confirm" => parsed.confirm = true,
            "--reduced-motion" => parsed.reduced_motion = true,
            "--scale" => {
                let scale: f32 = value()?
                    .parse()
                    .map_err(|_| "--scale must be a number".to_owned())?;
                if !(0.5..=4.0).contains(&scale) {
                    return Err("--scale must be between 0.5 and 4.0".to_owned());
                }
                parsed.scale = Some(scale);
            }
            "--verbose" => parsed.verbose = true,
            "--12h" => parsed.twelve_hour = true,
            "--no-date" => parsed.no_date = true,
            other => return Err(format!("unknown option '{other}'")),
        }
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_vec(args: &[&str]) -> Result<PreviewArgs, String> {
        parse(args.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn defaults_are_stable() {
        let args = parse_vec(&[]).unwrap();
        assert_eq!(args.scenario, PreviewScenario::Laptop);
        assert_eq!(args.width, 1600);
        assert_eq!(args.height, 1000);
        assert!(!args.fullscreen);
    }

    #[test]
    fn supports_inline_and_separate_values() {
        let args = parse_vec(&["--scenario=desktop", "--width", "1920", "--height=1080"]).unwrap();
        assert_eq!(args.scenario, PreviewScenario::Desktop);
        assert_eq!(args.width, 1920);
        assert_eq!(args.height, 1080);
    }

    #[test]
    fn rejects_unknown_scenarios_and_flags() {
        assert!(parse_vec(&["--scenario", "unknown"]).is_err());
        assert!(parse_vec(&["--not-a-flag"]).is_err());
    }

    #[test]
    fn rejects_out_of_range_sizes() {
        assert!(parse_vec(&["--width", "200"]).is_err());
        assert!(parse_vec(&["--height", "9000"]).is_err());
    }

    #[test]
    fn artifact_options_are_parsed() {
        let args = parse_vec(&[
            "--screenshot",
            "out.png",
            "--open-cc",
            "--preview",
            "browser",
            "--toast",
            "hello",
        ])
        .unwrap();
        assert!(args.open_cc);
        assert_eq!(args.preview_app.as_deref(), Some("browser"));
        assert_eq!(
            args.screenshot.as_deref(),
            Some(std::path::Path::new("out.png"))
        );
        assert_eq!(args.toast.as_deref(), Some("hello"));
    }
}
