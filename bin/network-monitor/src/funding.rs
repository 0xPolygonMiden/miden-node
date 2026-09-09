//! Fee funding for the monitor's accounts.
//!
//! Fees are withdrawn from the executing account's vault, so on a fee-charging chain the
//! monitor's fresh accounts need the fee asset before they can transact. This module requests a
//! P2ID note which holds the fee asset and returns it for consumption as an unauthenticated input
//! note.
//!
//! The funding service is the only source of the fee asset. The chain's faucet is not used for
//! fees, because a network does not always run a public faucet. The monitor still talks to the
//! faucet for its faucet checks, which is what [`FaucetClient`] is for.

use std::time::Duration;

use anyhow::{Context, Result};
use miden_node_proto::clients::FundingClient;
use miden_node_proto::domain::funding::RequestFunds;
use miden_node_proto::generated::funding_service::RequestFundsRequest as ProtoRequestFundsRequest;
use miden_node_tracing::info;
use miden_protocol::account::AccountId;
use miden_protocol::asset::Asset;
use miden_protocol::note::{Note, NoteInclusionProof};
use reqwest::Client;
use url::Url;

use crate::LOG_TARGET;
use crate::config::MonitorConfig;
use crate::faucet::{
    GetMetadataResponse,
    GetTokensResponse,
    fetch_faucet_metadata,
    request_tokens,
};
use crate::service::build_tls_client;

/// Upper bound on the fee formula's cycle multiplier: the kernel charges `verification_base_fee *
/// (ilog2(total_cycles) + 1)` with cycles capped at `2^29`.
const MAX_FEE_VERIFICATION_CYCLES: u64 = 30;

/// Increments one wallet funding request should cover, roughly a week at the default cadence.
const WALLET_FUNDING_INCREMENTS: u64 = 20_000;

/// Remaining-increment level at which the wallet requests a top-up.
const WALLET_TOPUP_THRESHOLD_INCREMENTS: u64 = 1_000;

/// Largest amount requested per funding call, matching the funding service's default
/// `--max-amount`. A larger request is rejected with `INVALID_ARGUMENT`.
const MAX_FUNDING_REQUEST_AMOUNT: u64 = 1_000_000_000;

/// Transactions the counter is funded for at deployment. It only pays its own creation fee from
/// this; later network transactions are paid by the sponsorship note each increment attaches. Kept
/// small: the counter allowlists P2ID at zero price, so third parties can drain this buffer with
/// dust notes, but increments keep working since sponsorships are collected before fees.
const COUNTER_FUNDING_TXS: u64 = 2;

/// Hard upper bound on one transaction's fee under the given base fee.
pub fn max_fee_per_transaction(verification_base_fee: u32) -> u64 {
    u64::from(verification_base_fee) * MAX_FEE_VERIFICATION_CYCLES
}

/// Cost of one increment to the wallet: its own fee plus the sponsorship note.
pub fn wallet_budget_per_increment(verification_base_fee: u32) -> u64 {
    max_fee_per_transaction(verification_base_fee) * 2
}

/// Amount requested when funding or topping up the wallet.
pub fn wallet_funding_amount(verification_base_fee: u32) -> u64 {
    (wallet_budget_per_increment(verification_base_fee) * WALLET_FUNDING_INCREMENTS)
        .min(MAX_FUNDING_REQUEST_AMOUNT)
}

/// Wallet balance below which a top-up is requested. Clamped to half the request cap so a capped
/// funding request still clears the threshold.
pub fn wallet_topup_threshold(verification_base_fee: u32) -> u64 {
    (wallet_budget_per_increment(verification_base_fee) * WALLET_TOPUP_THRESHOLD_INCREMENTS)
        .min(MAX_FUNDING_REQUEST_AMOUNT / 2)
}

/// Amount requested when funding the counter account at deployment.
pub fn counter_funding_amount(verification_base_fee: u32) -> u64 {
    (max_fee_per_transaction(verification_base_fee) * COUNTER_FUNDING_TXS)
        .min(MAX_FUNDING_REQUEST_AMOUNT)
}

/// HTTP client for the chain's faucet service, used by the monitor's faucet checks.
///
/// Wraps the token-request flow, which is a proof-of-work challenge plus `/get_tokens`, and the
/// metadata endpoint. The monitor does not pay fees from the faucet.
#[derive(Clone, Debug)]
pub struct FaucetClient {
    faucet_url: Url,
    client: Client,
    /// Wall-clock cap on solving a single faucet proof-of-work challenge.
    solve_timeout: Duration,
}

impl FaucetClient {
    pub fn new(faucet_url: Url, request_timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(request_timeout)
            .build()
            .expect("Failed to create HTTP client with timeout");
        Self {
            faucet_url,
            client,
            solve_timeout: request_timeout,
        }
    }

    /// Returns the faucet's base URL.
    pub fn url(&self) -> &Url {
        &self.faucet_url
    }

    /// Requests `amount` base units for `account_id`, solving the faucet's proof-of-work challenge.
    /// Does not wait for the minted note to commit.
    pub(crate) async fn request_tokens(
        &self,
        account_id: &str,
        amount: u64,
    ) -> Result<GetTokensResponse> {
        request_tokens(&self.client, &self.faucet_url, account_id, amount, self.solve_timeout).await
    }

    /// Fetches the faucet's metadata.
    pub(crate) async fn metadata(&self) -> Result<GetMetadataResponse> {
        fetch_faucet_metadata(&self.client, &self.faucet_url).await
    }
}

/// Builds the funding service client when its URL is configured.
///
/// Returns `None` when it is not, which is only usable on a chain that does not charge fees.
pub fn funding_client_from_config(config: &MonitorConfig) -> Option<FundingClient> {
    let url = config.funding_service_url.clone()?;

    Some(build_tls_client::<FundingClient>(url, config.funding_request_timeout))
}

/// Funds monitor accounts with the chain's fee asset.
///
/// Binds the funding service client to the chain's fee faucet ID, so callers fund an account from
/// just an ID and an amount. Built where the genesis header is known, since the fee faucet ID comes
/// from the genesis fee parameters.
pub struct FeeFunder {
    client: FundingClient,
    fee_faucet_id: AccountId,
}

impl FeeFunder {
    pub fn new(client: FundingClient, fee_faucet_id: AccountId) -> Self {
        Self { client, fee_faucet_id }
    }

    /// Requests `amount` base units for `account_id` and returns the committed P2ID note.
    ///
    /// The service answers only once the note is committed, so no lookup is needed here. The
    /// note's asset is checked against the fee faucet ID, so a service configured for another
    /// chain fails here instead of as opaque fee aborts later.
    pub async fn fund(&mut self, account_id: AccountId, amount: u64) -> Result<Note> {
        let request = RequestFunds { account_id, amount };
        let response = self
            .client
            .request_funds(ProtoRequestFundsRequest::from(request))
            .await
            .context("the funding service rejected the request")?
            .into_inner();

        let committed = response.note.context("the funding service returned no note")?;
        // The note is private, so this response holds the only copy of its details.
        let (note, _inclusion_proof) = <(Note, NoteInclusionProof)>::try_from(committed)
            .context("failed to convert the note of the funding service")?;

        ensure_note_carries_fee_asset(&note, self.fee_faucet_id).context(
            "the funding service did not send the chain's fee asset: is it configured for this \
             chain?",
        )?;

        info!(
            target: LOG_TARGET,
            "Received fee tokens from the funding service",
            account.id = account_id,
            note.id = note.id(),
            asset.amount = amount
        );

        Ok(note)
    }
}

/// Checks that the note holds a non-zero amount of the fee faucet's fungible asset.
fn ensure_note_carries_fee_asset(note: &Note, fee_faucet_id: AccountId) -> Result<()> {
    let funded = note.assets().iter().any(|asset| match asset {
        Asset::Fungible(asset) => asset.faucet_id() == fee_faucet_id && asset.amount().as_u64() > 0,
        Asset::NonFungible(_) => false,
    });
    anyhow::ensure!(
        funded,
        "note {} does not hold the fee asset issued by faucet {fee_faucet_id}",
        note.id().to_hex()
    );
    Ok(())
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use clap::Parser;
    use miden_protocol::Word;
    use miden_protocol::asset::FungibleAsset;
    use miden_protocol::note::NoteType;
    use miden_standards::note::P2idNote;

    use super::*;
    use crate::deploy::wallet::create_wallet_account;

    /// Parses a monitor configuration which holds the given arguments and nothing else of interest.
    fn config_with(arguments: &[&str]) -> MonitorConfig {
        let mut command = vec!["miden-network-monitor", "--rpc-url", "http://rpc.invalid"];
        command.extend_from_slice(arguments);
        MonitorConfig::parse_from(command)
    }

    /// Requested amounts must stay within the funding service's maximum, and a capped request must
    /// still clear the top-up threshold.
    #[test]
    fn funding_amounts_respect_the_request_maximum() {
        for base_fee in [1, 500, 834, 10_000, u32::MAX] {
            assert!(wallet_funding_amount(base_fee) <= MAX_FUNDING_REQUEST_AMOUNT);
            assert!(counter_funding_amount(base_fee) <= MAX_FUNDING_REQUEST_AMOUNT);
            assert!(
                wallet_funding_amount(base_fee) >= 2 * wallet_topup_threshold(base_fee),
                "a single funding request must cover at least two thresholds at base fee \
                 {base_fee}"
            );
        }

        // Small base fees stay on the uncapped formula.
        assert_eq!(
            wallet_funding_amount(1),
            wallet_budget_per_increment(1) * WALLET_FUNDING_INCREMENTS
        );
        // Large base fees hit the cap instead of producing a rejected request.
        assert_eq!(wallet_funding_amount(10_000), MAX_FUNDING_REQUEST_AMOUNT);
    }

    /// Fees come from the funding service only, so a configured faucet must not produce a funding
    /// client.
    ///
    /// Runs on a Tokio runtime because building the lazy gRPC client needs one.
    #[tokio::test]
    async fn only_the_funding_service_url_provides_fee_funding() {
        let faucet_url = Url::parse("http://faucet.invalid").expect("static URL is valid");
        let service_url = Url::parse("http://funding.invalid").expect("static URL is valid");

        let service = config_with(&["--funding-service-url", service_url.as_str()]);
        assert!(funding_client_from_config(&service).is_some());

        let faucet_only = config_with(&["--faucet-url", faucet_url.as_str()]);
        assert!(
            funding_client_from_config(&faucet_only).is_none(),
            "the faucet must not be used as a source of fees"
        );

        let neither = config_with(&[]);
        assert!(
            funding_client_from_config(&neither).is_none(),
            "without the funding service the chain must not charge fees"
        );
    }

    /// A source sending the wrong token must fail at claim time, not as later fee aborts.
    #[test]
    fn funding_note_must_carry_the_fee_asset() {
        let fee_faucet_id = FungibleAsset::mock_issuer();
        let (wallet, _secret_key) = create_wallet_account().expect("wallet account should build");
        let note: Note = P2idNote::builder()
            .sender(fee_faucet_id)
            .target(wallet.id())
            .serial_number(Word::from([7u32; 4]))
            .note_type(NoteType::Public)
            .asset(FungibleAsset::new(fee_faucet_id, 100).expect("valid asset"))
            .build()
            .expect("the note should build")
            .into();

        ensure_note_carries_fee_asset(&note, fee_faucet_id)
            .expect("a note carrying the fee asset must be accepted");
        // Any other issuer id must be rejected; the wallet id stands in for a wrong faucet.
        ensure_note_carries_fee_asset(&note, wallet.id())
            .expect_err("a note without the fee asset must be rejected");
    }
}
