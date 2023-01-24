use serde::{Deserialize, Serialize};

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
