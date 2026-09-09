use miden_node_proto::domain::funding::RequestFunds as RequestFundsInput;
use miden_node_proto::generated as proto;
use miden_node_proto::server::funding_service_api;
use tokio::sync::{mpsc, oneshot};

use super::FundingRpcServer;
use crate::COMPONENT;
use crate::error::RequestFundsError;
use crate::worker::{FundedNote, FundingRequest};

#[tonic::async_trait]
impl funding_service_api::RequestFunds for FundingRpcServer {
    type Input = RequestFundsInput;
    type Output = FundedNote;

    fn decode(request: proto::funding_service::RequestFundsRequest) -> tonic::Result<Self::Input> {
        RequestFundsInput::try_from(request).map_err(Into::into)
    }

    #[miden_node_tracing::miden_instrument(
        target = COMPONENT,
        name = "request_funds",
        fields (
            account.id = request.account_id,
            asset.amount = request.amount,
        ),
        err,
    )]
    async fn handle(
        &self,
        request: Self::Input,
        _metadata: &tonic::metadata::MetadataMap,
        _extensions: &tonic::codegen::http::Extensions,
    ) -> tonic::Result<Self::Output> {
        self.validate_amount(request.amount)?;

        let (reply, response) = oneshot::channel();
        let queued = FundingRequest {
            target: request.account_id,
            amount: request.amount,
            reply,
        };

        self.requests.try_send(queued).map_err(|err| match err {
            mpsc::error::TrySendError::Full(_) => RequestFundsError::Busy,
            mpsc::error::TrySendError::Closed(_) => {
                RequestFundsError::NotReady("the funding worker stopped")
            },
        })?;

        // The worker answers once the note is committed, which takes at least one block interval. A
        // dropped sender means the worker stopped without answering.
        response
            .await
            .map_err(|_| RequestFundsError::NotReady("the funding worker stopped"))?
            .map_err(Into::into)
    }

    fn encode(funded: Self::Output) -> tonic::Result<proto::funding_service::RequestFundsResponse> {
        let FundedNote { note, inclusion_proof, transaction_id } = funded;

        Ok(proto::funding_service::RequestFundsResponse {
            note: Some((note, inclusion_proof).into()),
            transaction_id: Some(transaction_id.into()),
        })
    }
}

impl FundingRpcServer {
    /// Checks the requested amount against the configured maximum.
    fn validate_amount(&self, amount: u64) -> Result<(), RequestFundsError> {
        if amount == 0 {
            return Err(RequestFundsError::InvalidAmount);
        }

        let maximum = self.status.max_amount();
        if amount > maximum {
            return Err(RequestFundsError::AmountExceedsMaximum { requested: amount, maximum });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use miden_node_proto::generated::account::AccountId;
    use miden_node_proto::generated::funding_service::RequestFundsRequest;
    use miden_protocol::Word;
    use miden_protocol::asset::FungibleAsset;
    use miden_protocol::crypto::merkle::{MerklePath, SparseMerklePath};
    use miden_protocol::note::{NoteInclusionProof, NoteType};
    use miden_standards::note::P2idNote;

    use super::*;
    use crate::server::tests::test_server;

    const MAX_AMOUNT: u64 = 1_000;

    #[test]
    fn decode_rejects_a_missing_account_id() {
        let err =
            <FundingRpcServer as funding_service_api::RequestFunds>::decode(RequestFundsRequest {
                account_id: None,
                amount: 1,
            })
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn decode_rejects_a_malformed_account_id() {
        let err =
            <FundingRpcServer as funding_service_api::RequestFunds>::decode(RequestFundsRequest {
                account_id: Some(AccountId { id: vec![1, 2, 3] }),
                amount: 1,
            })
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn a_zero_amount_is_rejected() {
        let (server, _rx) = test_server(MAX_AMOUNT);
        let err = server.validate_amount(0).unwrap_err();
        assert_eq!(tonic::Status::from(err).details(), [1]);
    }

    #[test]
    fn an_amount_above_the_maximum_is_rejected() {
        let (server, _rx) = test_server(MAX_AMOUNT);

        server.validate_amount(MAX_AMOUNT).expect("the maximum itself is allowed");
        let err = server.validate_amount(MAX_AMOUNT + 1).unwrap_err();
        assert_eq!(tonic::Status::from(err).details(), [2]);
    }

    /// The response must carry the note in full: the node does not store the details of a private
    /// note, so the requester cannot fetch them anywhere else.
    #[test]
    fn encode_round_trips_a_private_note_with_its_proof() {
        let faucet_id = FungibleAsset::mock_issuer();
        let note: miden_protocol::note::Note = P2idNote::builder()
            .sender(faucet_id)
            .target(faucet_id)
            .serial_number(Word::from([3u32; 4]))
            .note_type(NoteType::Private)
            .asset(FungibleAsset::new(faucet_id, 42).unwrap())
            .build()
            .unwrap()
            .into();
        let proof = NoteInclusionProof::new(
            7.into(),
            3,
            SparseMerklePath::try_from(MerklePath::new(vec![Word::from([1u32; 4])])).unwrap(),
        )
        .unwrap();
        let transaction_id =
            miden_protocol::transaction::TransactionId::from_raw(Word::from([9u32; 4]));

        let response =
            <FundingRpcServer as funding_service_api::RequestFunds>::encode(FundedNote {
                note: note.clone(),
                inclusion_proof: proof.clone(),
                transaction_id,
            })
            .unwrap();

        let committed = response.note.expect("the response holds the note");
        let (decoded_note, decoded_proof) =
            <(miden_protocol::note::Note, NoteInclusionProof)>::try_from(committed).unwrap();

        assert_eq!(decoded_note.id(), note.id());
        assert_eq!(decoded_note.assets(), note.assets());
        assert_eq!(decoded_note.recipient(), note.recipient());
        assert_eq!(decoded_proof.location(), proof.location());
        assert_eq!(
            miden_protocol::transaction::TransactionId::try_from(response.transaction_id.unwrap())
                .unwrap(),
            transaction_id
        );
    }
}
