use clap::Parser;
use directories::UserDirs;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    #[arg()]
    pub file: Option<String>,

    #[clap(long, env="LOX_HISTORY_FILE", default_value = get_default_history_file().into_os_string())]
    pub history_file: PathBuf,

    /// Verbose debug information
    #[clap(short, long, default_value_t)]
    pub verbose: bool,

    /// Garbage collection logs
    #[clap(short, long, default_value_t)]
    pub gc_log: bool,
}

fn get_default_history_file() -> PathBuf {
    UserDirs::new()
        .map(|dirs| dirs.home_dir().join(".lox_history"))
        .unwrap()
}
