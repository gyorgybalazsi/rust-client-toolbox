use crate::archive::archive_from_dar;
use anyhow::{Context, Result};
use crate::lf_protobuf::com::daml::daml_lf_2::Package;
use crate::lf_protobuf::com::daml::daml_lf_dev::ArchivePayload;
use prost::Message;


pub fn package_from_dar(path: &str) -> Result<Package> {
    let archive = archive_from_dar(path)
        .with_context(|| format!("Failed to read archive from '{}'", path))?;

    let payload = ArchivePayload::decode(&*archive.payload)
        .with_context(|| "Failed to decode ArchivePayload")?;

    if let Some(crate::lf_protobuf::com::daml::daml_lf_dev::archive_payload::Sum::DamlLf2(
        dalf_bytes,
    )) = payload.sum
    {
        let package = crate::lf_protobuf::com::daml::daml_lf_2::Package::decode(&*dalf_bytes)
            .with_context(|| "Failed to decode Package from DALF bytes")?;
        Ok(package)
    } else {
        anyhow::bail!("Expected DamlLf2 variant in ArchivePayload");
    }
}