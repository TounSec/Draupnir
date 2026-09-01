use std::path::PathBuf;

pub struct Config {
    pub sources: Vec<PathBuf>,
    pub destination: PathBuf,
    pub age_pubkey: String,
    pub keep: usize,
}
