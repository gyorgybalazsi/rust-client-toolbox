use ledger_api::v2::{
    GetLedgerEndRequest, GetLedgerEndResponse, state_service_client::StateServiceClient,
};
use tonic::Request;
use tonic::metadata::MetadataValue;
use anyhow::Result;

pub async fn get_ledger_end(
    url: &str,
    access_token: Option<&str>,
) -> Result<i64> {
    let mut state_service_client = StateServiceClient::connect(url.to_string()).await?;
    let mut req: Request<GetLedgerEndRequest> = Request::new(GetLedgerEndRequest {});
    if let Some(token) = access_token {
        let meta = MetadataValue::try_from(format!("Bearer {}", token))?;
        req.metadata_mut().insert("authorization", meta);
    }
    let ledger_end_response: tonic::Response<GetLedgerEndResponse> =
        state_service_client.get_ledger_end(req).await?;
    Ok(ledger_end_response.into_inner().offset)
}
