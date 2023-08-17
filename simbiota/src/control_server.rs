use crate::detection_system;
use crate::detection_system::Action::QueryQuarantine;
use crate::detection_system::{Action, CommandResult, DetectionSystem, DetectorCommand};
use crossbeam_channel::{Receiver, Sender};
use libc::c_char;
use log::{debug, error, info};
use simbiota_protocol::{Command, CommandRequest, CommandResponse, CommandStatus, Response};
use std::ffi::CString;
use std::io::{BufRead, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct ControlServer {
    listener: UnixListener,
    client_tx: Sender<detection_system::DetectorCommand>,
    client_id: usize,
    client_rx: Receiver<detection_system::CommandResult>,
}

impl ControlServer {
    pub fn new(com: (usize, Receiver<CommandResult>, Sender<DetectorCommand>)) -> Self {
        // check whether we can connect
        let connection = UnixStream::connect_addr(&simbiota_protocol::socket_address());
        if let Ok(_) = connection {
            error!("cannot start control server: already running");
            eprintln!("Anothe instance of SIMBIoTA is already running");
            exit(1);
        }

        /*unsafe {
            let path = CString::new("/var/run/simbiota.sock").unwrap();
            libc::unlink(path.as_ptr() as *const c_char);
        }*/
        let listener = UnixListener::bind_addr(&simbiota_protocol::socket_address())
            .expect("Failed to bind to socket");

        Self {
            listener,
            client_id: com.0,
            client_rx: com.1,
            client_tx: com.2,
        }
    }

    pub fn listen(&self) -> ! {
        info!("control server listening");
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    debug!("control connection: {:?}", stream);
                    self.serve(stream);
                }
                Err(err) => {
                    error!("error: {:?}", err);
                }
            }
        }
        panic!("Control server stopped listening");
    }

    fn serve(&self, mut stream: std::os::unix::net::UnixStream) {
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let mut reader = std::io::BufReader::new(&stream);
        let mut writer = std::io::BufWriter::new(&stream);
        let mut command_line = String::new();
        reader.read_line(&mut command_line).unwrap();
        if command_line.is_empty() {
            return;
        }
        let Ok(command) = serde_json::from_str::<CommandRequest>(&command_line) else {
            error!("failed to parse command: {:?}", command_line);
            return;
        };
        debug!("control request: {:?}", command);

        fn failure(msg: &str) -> CommandResponse {
            CommandResponse {
                status: CommandStatus::Failure(msg.to_string()),
                response: Response::None,
            }
        }

        let result: CommandResponse = match command.command {
            Command::ManualScan { .. } => CommandResponse {
                status: CommandStatus::Failure("not implemented".to_string()),
                response: Response::None,
            },
            Command::ManualScanStatus => {
                todo!("not supported");
            }
            Command::ManualScanCancel => {
                todo!("not supported");
            }
            Command::QueryQuarantine => {
                self.client_tx
                    .send(DetectorCommand {
                        id: self.client_id,
                        command: Action::QueryQuarantine,
                    })
                    .unwrap();
                let result = self.client_rx.recv().unwrap();
                match result {
                    CommandResult::QuarantineEntries(entries) => CommandResponse {
                        status: CommandStatus::Success,
                        response: Response::QuarantineQueryResponse(
                            entries
                                .iter()
                                .enumerate()
                                .map(|(i, e)| (i, e.original_path.clone()))
                                .collect(),
                        ),
                    },
                    _ => failure("invalid response from detector"),
                }
            }
            Command::RestoreQuarantine(e) => {
                self.client_tx
                    .send(DetectorCommand {
                        id: self.client_id,
                        command: Action::RestoreQuarantineEntry(e),
                    })
                    .unwrap();
                let result = self.client_rx.recv().unwrap();
                match result {
                    CommandResult::QuarantineAction(s) => CommandResponse {
                        status: CommandStatus::Success,
                        response: Response::QuarantineActionResponse(s),
                    },
                    _ => failure("invalid response from detector"),
                }
            }
            Command::DeleteQuarantine(e) => {
                self.client_tx
                    .send(DetectorCommand {
                        id: self.client_id,
                        command: Action::DeleteQuarantineEntry(e),
                    })
                    .unwrap();

                let result = self.client_rx.recv().unwrap();
                match result {
                    CommandResult::QuarantineAction(s) => CommandResponse {
                        status: CommandStatus::Success,
                        response: Response::QuarantineActionResponse(s),
                    },
                    _ => failure("invalid response from detector"),
                }
            }
            Command::Restart => {
                todo!("not supported");
            }
        };
        let response = serde_json::to_string(&result).unwrap();
        writer.write_all(response.as_bytes()).unwrap();
        writer.write_all("\n".as_bytes()).unwrap();
    }
}
