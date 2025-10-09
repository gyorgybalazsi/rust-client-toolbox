use std::error;
use std::fs;
use std::io::Error;
use std::path::Path;
use std::path::PathBuf;

const ALL_PROTO_SRC_PATHS: &[&str] = &[
    "com/daml/ledger/api/v2",
    "com/daml/ledger/api/v2/testing",
    "com/daml/ledger/api/v2/admin",
    "com/daml/ledger/api/v2/interactive",
    "google/protobuf",
    "google/rpc",
];
const PROTO_ROOT_PATH: &str = "resources/protobuf";

fn main() -> Result<(), Box<dyn error::Error>> {
    let all_protos = get_all_protos(ALL_PROTO_SRC_PATHS)?;
    tonic_build::configure()
        .type_attribute("com.daml.ledger.api.v2.Record", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.RecordField", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.Identifier", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.Value", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.Value.sum", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.Optional", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.List", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.TextMap", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.TextMap.Entry", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.GenMap", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.GenMap.Entry", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.Variant", "#[derive(serde::Serialize)]")
        .type_attribute("com.daml.ledger.api.v2.Enum", "#[derive(serde::Serialize)]")
        .build_server(false)
        .build_client(true)
        .out_dir("src/pb")
        .compile_protos(
            &all_protos,
            &[PROTO_ROOT_PATH],
        )?;
    Ok(())
}

fn get_all_protos(src_paths: &[&str]) -> Result<Vec<PathBuf>, Error> {
    let mut protos = Vec::new();
    for path in src_paths {
        let dir = Path::new(path);
        let files = get_protos_from_dir(dir)?;
        protos.extend(files);
    }
    Ok(protos)
}

fn get_protos_from_dir(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    fs::read_dir(Path::new(PROTO_ROOT_PATH).join(dir))?
        .filter_map(|entry| match entry {
            Ok(d) => match d.path().extension() {
                Some(a) if a == "proto" => Some(Ok(d.path())),
                _ => None,
            },
            Err(e) => Some(Err(e)),
        })
        .collect()
}


