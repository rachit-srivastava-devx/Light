use clap::{Parser, Subcommand};
use std::ffi::OsString;
use crate::{CliCommand, CliError};

const MAX_BYTES: usize = 65536;

#[derive(Parser)]
#[command(name = "user-cli")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Submit { request: String },
    Pause,
    Resume,
    Cancel,
    Approve { grant: String },
    Status,
}

pub fn parse<I, T>(args: I) -> Result<CliCommand, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let argv: Vec<OsString> = std::iter::once(OsString::from("user-cli"))
        .chain(args.into_iter().map(|a| a.into()))
        .collect();
    let cli = Cli::try_parse_from(argv).map_err(|_| CliError::UnknownCommand)?;
    match cli.command {
        Cmd::Submit { request } => {
            let bytes = request.len();
            if bytes == 0 {
                return Err(CliError::EmptyRequest);
            }
            if bytes > MAX_BYTES {
                return Err(CliError::TooLarge { bytes });
            }
            Ok(CliCommand::Submit { request })
        }
        Cmd::Pause => Ok(CliCommand::Pause),
        Cmd::Resume => Ok(CliCommand::Resume),
        Cmd::Cancel => Ok(CliCommand::Cancel),
        Cmd::Approve { grant } => {
            if grant.is_empty() {
                return Err(CliError::InvalidGrant);
            }
            Ok(CliCommand::Approve { grant })
        }
        Cmd::Status => Ok(CliCommand::Status),
    }
}
