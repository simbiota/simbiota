use serde::{Deserialize, Serialize};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::SocketAddr;

pub fn socket_address() -> SocketAddr {
    SocketAddr::from_abstract_name("simbiota").unwrap()
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    ManualScan { path: String, recursive: bool },
    ManualScanStatus,
    ManualScanCancel,

    QueryQuarantine,
    RestoreQuarantine(String),
    DeleteQuarantine(String),

    Restart,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CommandStatus {
    Success,
    Failure(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    None,
    QuarantineQueryResponse(Vec<(usize, String)>),
    QuarantineActionResponse(bool),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandRequest {
    pub command: Command,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResponse {
    pub status: CommandStatus,
    pub response: Response,
}
