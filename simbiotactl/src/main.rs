use crate::cli::{Cli, QuarantineCommand, Subsys};
use clap::Parser;
use control_protocol::{Command, CommandRequest, CommandResponse, Response};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::exit;

mod cli;

fn main() {
    let cli = Cli::parse();

    let connection = UnixStream::connect("/var/run/simbiota.sock");
    if let Err(e) = connection {
        eprintln!("failed to connect to control socket: {:?}", e.to_string());
        exit(1);
    }
    let mut connection = connection.unwrap();

    let output = match cli.subsys {
        /*Subsys::Scan { command } => match command {
            ScanCommand::Start { path, recursive } => {
                let command = CommandRequest {
                    command: Command::ManualScan {
                        path: path.to_string_lossy().to_string(),
                        recursive,
                    },
                };
                serde_json::to_string(&command).unwrap()
            }
            ScanCommand::List => String::new(),
            ScanCommand::Cancel { .. } => String::new(),
        },*/
        Subsys::Quarantine { command } => match command {
            QuarantineCommand::List => {
                let command = CommandRequest {
                    command: Command::QueryQuarantine,
                };
                serde_json::to_string(&command).unwrap()
            }
            QuarantineCommand::Restore { id_or_path } => {
                let command = CommandRequest {
                    command: Command::RestoreQuarantine(id_or_path),
                };
                serde_json::to_string(&command).unwrap()
            }
            QuarantineCommand::Delete { id_or_path } => {
                let command = CommandRequest {
                    command: Command::DeleteQuarantine(id_or_path),
                };
                serde_json::to_string(&command).unwrap()
            }
        },
    };
    connection.write_all(output.as_ref()).unwrap();
    connection.write_all("\n".as_ref()).unwrap();
    connection.flush().unwrap();

    let mut response_bytes = vec![];
    connection
        .read_to_end(&mut response_bytes)
        .expect("failed to read response");

    let response: CommandResponse =
        serde_json::from_slice(&response_bytes).expect("invalid response");
    if let control_protocol::CommandStatus::Failure(reason) = response.status {
        eprintln!("command failed: {}", reason);
    } else {
        match response.response {
            Response::None => {}
            Response::QuarantineQueryResponse(e) => {
                if e.is_empty() {
                    println!("Quarantine is empty");
                    return;
                }

                println!("Quarantine entries:");
                for entry in e {
                    println!("\t{}:\t{}", entry.0, entry.1);
                }
            }
            Response::QuarantineActionResponse(s) => {
                if s {
                    println!("Quarantine action succeeded");
                } else {
                    println!("Quarantine action failed");
                }
            }
        }
    }
}
