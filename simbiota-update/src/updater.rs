use log::info;
use std::fs::OpenOptions;
use std::io::{Error, ErrorKind};
use std::path::Path;

pub(crate) fn perform_update<P>(
    database_path: P,
    server: String,
    arch: String,
) -> Result<(), std::io::Error>
where
    P: AsRef<Path>,
{
    let url = format!("http://{server}/update/{arch}");

    let client = reqwest::blocking::ClientBuilder::new()
        .build()
        .map_err(|e| Error::new(ErrorKind::Other, e))?;

    info!("downloading update from: {url}");
    let request = client
        .get(url)
        .build()
        .map_err(|e| Error::new(ErrorKind::Other, e))?;
    let response = client
        .execute(request)
        .map_err(|e| Error::new(ErrorKind::Other, e))?;

    let mut out_file = OpenOptions::new().write(true).open(database_path)?;
    std::io::copy(
        &mut &*(response
            .bytes()
            .map_err(|e| Error::new(ErrorKind::Other, e))?),
        &mut out_file,
    )?;

    info!("done");
    Ok(())
}
