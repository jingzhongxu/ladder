use super::error::{self, ResultExt};
use super::SuperviseClient;
use crate::events;
use crate::message::RelayMessage;
use crate::state::State;
use contracts;
use futures::{Async, Poll, Stream};
use log_stream::{ChainAlias, LogStream, LogStreamOptions};
use std::sync::Arc;
use std::time::Duration;
use web3::{types::Address, Transport};

/// vendor will listen to all preset event.
/// it submit event when poll finished, repeat event will be discarded.
pub struct Vendor<T: Transport, C: SuperviseClient> {
    client: Arc<C>,
    state: State,
    ingress_stream: LogStream<T>,
    egress_stream: LogStream<T>,
    deposit_stream: LogStream<T>,
    withdraw_stream: LogStream<T>,
    authority_stream: LogStream<T>,
}

impl<T: Transport, C: SuperviseClient> Vendor<T, C> {
    pub fn new(
        transport: &T,
        client: Arc<C>,
        state: State,
        contract_address: Address,
        chain: ChainAlias,
    ) -> Self {
        Self {
            ingress_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(30),
                poll_interval: Duration::from_secs(10),
                confirmations: 1,
                transport: transport.clone(),
                contract_address: contract_address,
                last_block_number: state.ingress, //10351660,
                filter: contracts::bridge::events::ingress::filter(),
                chain,
            }),
            egress_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(30),
                poll_interval: Duration::from_secs(10),
                confirmations: 1,
                transport: transport.clone(),
                contract_address: contract_address,
                last_block_number: state.egress,
                filter: contracts::bridge::events::egress::filter(),
                chain,
            }),
            deposit_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(30),
                poll_interval: Duration::from_secs(10),
                confirmations: 1,
                transport: transport.clone(),
                contract_address: contract_address,
                last_block_number: state.deposit,
                filter: contracts::bridge::events::deposit::filter(),
                chain,
            }),
            withdraw_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(30),
                poll_interval: Duration::from_secs(10),
                confirmations: 1,
                transport: transport.clone(),
                contract_address: contract_address,
                last_block_number: state.withdraw,
                filter: contracts::bridge::events::withdraw::filter(),
                chain,
            }),
            authority_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(30),
                poll_interval: Duration::from_secs(10),
                confirmations: 1,
                transport: transport.clone(),
                contract_address: contract_address,
                last_block_number: state.authority,
                filter: contracts::bridge::events::replace_auths::filter(),
                chain,
            }),
            client: client,
            state: state,
        }
    }

    pub fn mock(transport: &T, client: Arc<C>) -> Self {
        Self {
            ingress_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(1),
                poll_interval: Duration::from_secs(1),
                confirmations: 12,
                transport: transport.clone(),
                contract_address: "0000000000000000000000000000000000000001".into(),
                last_block_number: 3,
                filter: contracts::bridge::events::ingress::filter(),
                chain: ChainAlias::ETH,
            }),
            egress_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(1),
                poll_interval: Duration::from_secs(1),
                confirmations: 12,
                transport: transport.clone(),
                contract_address: "0000000000000000000000000000000000000002".into(),
                last_block_number: 3,
                filter: contracts::bridge::events::egress::filter(),
                chain: ChainAlias::ETH,
            }),
            deposit_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(1),
                poll_interval: Duration::from_secs(1),
                confirmations: 12,
                transport: transport.clone(),
                contract_address: "0000000000000000000000000000000000000002".into(),
                last_block_number: 3,
                filter: contracts::bridge::events::deposit::filter(),
                chain: ChainAlias::ETH,
            }),
            withdraw_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(1),
                poll_interval: Duration::from_secs(1),
                confirmations: 12,
                transport: transport.clone(),
                contract_address: "0000000000000000000000000000000000000002".into(),
                last_block_number: 3,
                filter: contracts::bridge::events::withdraw::filter(),
                chain: ChainAlias::ETH,
            }),
            authority_stream: LogStream::new(LogStreamOptions {
                request_timeout: Duration::from_secs(1),
                poll_interval: Duration::from_secs(1),
                confirmations: 12,
                transport: transport.clone(),
                contract_address: "0000000000000000000000000000000000000002".into(),
                last_block_number: 3,
                filter: contracts::bridge::events::replace_auths::filter(),
                chain: ChainAlias::ETH,
            }),
            client: client,
            state: State::default(),
        }
    }
}

impl<T: Transport, C: SuperviseClient> Stream for Vendor<T, C> {
    type Item = State;
    type Error = error::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            let mut changed = false;
            // get ingress logs
            let ret = try_maybe_stream!(self
                .ingress_stream
                .poll()
                .chain_err(|| "Vendor: Get poll log Failed.",));
            if let Some(ret) = ret {
                for log in &ret.logs {
                    let message = events::IngressEvent::from_log(log)?;
                    self.client.submit(RelayMessage::from(message));
                }
                self.state.ingress = ret.to;
                changed = true;
            }

            let ret = try_maybe_stream!(self
                .egress_stream
                .poll()
                .chain_err(|| "Vendor: Get poll log Failed.",));
            if let Some(ret) = ret {
                for log in &ret.logs {
                    let message = events::EgressEvent::from_log(log)?;
                    self.client.submit(RelayMessage::from(message));
                }
                self.state.egress = ret.to;
                changed = true;
            }

            let ret = try_maybe_stream!(self
                .deposit_stream
                .poll()
                .chain_err(|| "Vendor: Get poll log Failed.",));
            if let Some(ret) = ret {
                for log in &ret.logs {
                    let message = events::DepositEvent::from_log(log)?;
                    self.client.submit(RelayMessage::from(message));
                }
                self.state.deposit = ret.to;
                changed = true;
            }

            let ret = try_maybe_stream!(self
                .withdraw_stream
                .poll()
                .chain_err(|| "Vendor: Get poll log Failed.",));
            if let Some(ret) = ret {
                for log in &ret.logs {
                    let message = events::WithdrawEvent::from_log(log)?;
                    self.client.submit(RelayMessage::from(message));
                }
                self.state.withdraw = ret.to;
                changed = true;
            }

            let ret = try_maybe_stream!(self
                .authority_stream
                .poll()
                .chain_err(|| "Vendor: Get poll log Failed.",));
            if let Some(ret) = ret {
                for log in &ret.logs {
                    let message = events::AuthorityEvent::from_log(log)?;
                    self.client.submit(RelayMessage::from(message));
                }
                self.state.authority = ret.to;
                changed = true;
            }

            if changed {
                return Ok(Async::Ready(Some(self.state.clone())));
            } else {
                return Ok(Async::NotReady);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts;
    use rustc_hex::FromHex;
    use test::MockClient;
    use tokio_core::reactor::Core;
    use utils::StreamExt;
    use web3::types::{Bytes, Log};

    #[test]
    fn test_vendor_stream() {
        let ingress_topic = contracts::bridge::events::ingress::filter().topic0;

        let client = Arc::new(MockClient::default());
        let transport = mock_transport!(
            "eth_blockNumber" =>
                req => json!([]),
                res => json!("0x1011");
            "eth_getLogs" =>
                req => json!([{
                    "address": "0x0000000000000000000000000000000000000001",
                    "fromBlock": "0x4",
                    "toBlock": "0x1005",
                    "topics": [ingress_topic]
                }]),
                res => json!([{
                    "address": "0x0000000000000000000000000000000000000001",
                    "topics": [ingress_topic],
                    "data": "0x000000000000000000000000000000000000000000000000000000000000000200000000000000000000000074241db5f3ebaeecf9506e4ae98818609334160400000000000000000000000074241db5f3ebaeecf9506e4ae98818609334160400000000000000000000000000000000000000000000000000000000054c5638",
                    "type": "",
                    "transactionHash": "0x1045bfe274b88120a6b1e5d01b5ec00ab5d01098346e90e7c7a3c9b8f0181c80"
                }]);
            "eth_blockNumber" =>
                req => json!([]),
                res => json!("0x1012");
            "eth_getLogs" =>
                req => json!([{
                    "address": "0x0000000000000000000000000000000000000001",
                    "fromBlock": "0x1006",
                    "toBlock": "0x1006",
                    "topics": [ingress_topic]
                }]),
                res => json!([]);
        );
        let vendor = Vendor::mock(&transport, client.clone());
        let mut event_loop = Core::new().unwrap();
        let log_ranges = event_loop.run(vendor.take(2).collect()).unwrap();
    }
}
