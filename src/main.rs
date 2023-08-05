/*
 * Copyright (C) 2023 K4YT3X.
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License
 * as published by the Free Software Foundation; only version 2
 * of the License.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

use std::{fs, path::PathBuf, process};

use anyhow::Result;
use aufseher::{run, AufseherConfig, Config};
use clap::Parser;
use tracing::Level;
use tracing_subscriber;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Telegram bot API token
    #[arg(short = 't', long, env = "TELEGRAM_BOT_TOKEN", required = true)]
    token: String,

    /// Path to config file
    #[arg(short = 'c', long, default_value = "configs/aufseher.yaml")]
    config_file: PathBuf,
}

fn parse() -> Result<Config> {
    let args = Args::parse();

    // Read config file
    let file_contents = fs::read_to_string(args.config_file)?;
    let regex_config: AufseherConfig = serde_yaml::from_str(&file_contents)?;

    Ok(Config::new(args.token, regex_config))
}

#[tokio::main]
async fn main() {
    // Setup tracing.
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    match parse() {
        Err(e) => {
            eprintln!("Program initialization error: {}", e);
            process::exit(1);
        }
        Ok(config) => process::exit(match run(config).await {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        }),
    }
}
