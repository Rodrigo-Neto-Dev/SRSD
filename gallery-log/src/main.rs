mod crypto;
mod log;
mod logappend;
mod logread;
mod state;

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Append {
        #[arg(short)]
        t: u64,
        #[arg(short)]
        k: String,
        #[arg(short)]
        e: Option<String>,
        #[arg(short)]
        g: Option<String>,
    },
    Read {
        #[arg(short)]
        k: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Append { t, k, e, g } => {
            println!("Append not fully implemented yet");
        }
        Commands::Read { k } => {
            println!("Read not fully implemented yet");
        }
    }
}