use crate::block_number_stream::{BlockNumberStream, BlockNumberStreamOptions};
use error::{self, ResultExt};
//use crate::error::ResultExt;
//use crate::error;
use ethabi;
use futures::future::FromErr;
use futures::{Async, Future, Poll, Stream};
use std::time::Duration;
use tokio_timer::{Timeout, Timer};
use web3;
use web3::api::Namespace;
use web3::helpers::CallFuture;
use web3::types::{Address, Filter, FilterBuilder, Log, H256};
use web3::Transport;

pub fn abos_logs<T: Transport>(transport: &T, filter: Filter) -> CallFuture<Vec<Log>, T::Out> {
    let filter = web3::helpers::serialize(&filter);
    CallFuture::new(transport.execute("getLogs", vec![filter]))
}

fn ethabi_topic_to_web3(topic: &ethabi::Topic<ethabi::Hash>) -> Option<Vec<H256>> {
    match topic {
        ethabi::Topic::Any => None,
        ethabi::Topic::OneOf(options) => Some(options.clone()),
        ethabi::Topic::This(hash) => Some(vec![hash.clone()]),
    }
}

fn filter_to_builder(filter: &ethabi::TopicFilter, address: Address) -> FilterBuilder {
    let t0 = ethabi_topic_to_web3(&filter.topic0);
    let t1 = ethabi_topic_to_web3(&filter.topic1);
    let t2 = ethabi_topic_to_web3(&filter.topic2);
    let t3 = ethabi_topic_to_web3(&filter.topic3);
    FilterBuilder::default()
        .address(vec![address])
        .topics(t0, t1, t2, t3)
}

/// options for creating a `LogStream`. passed to `LogStream::new`
pub struct LogStreamOptions<T> {
    pub filter: ethabi::TopicFilter,
    pub request_timeout: Duration,
    pub poll_interval: Duration,
    pub confirmations: u32,
    pub transport: T,
    pub contract_address: Address,
    pub last_block_number: u64,
    pub chain: ChainAlias,
}

/// Contains all logs matching `LogStream` filter in inclusive block range `[from, to]`.
#[derive(Debug, PartialEq)]
pub struct LogsInBlockRange {
    pub from: u64,
    pub to: u64,
    pub logs: Vec<Log>,
}

/// Log Stream state.
#[derive(Debug)]
enum State<T: Transport> {
    /// Fetching best block number.
    AwaitBlockNumber,
    /// Fetching logs for new best block.
    AwaitLogs {
        from: u64,
        to: u64,
        future: Timeout<FromErr<CallFuture<Vec<Log>, T::Out>, error::Error>>,
    },
}

/// Alias of Chain to diff.
#[derive(Copy, Clone)]
pub enum ChainAlias {
    ETH,
    ABOS,
}

pub struct LogStream<T: Transport> {
    block_number_stream: BlockNumberStream<T>,
    request_timeout: Duration,
    transport: T,
    last_checked_block: u64,
    timer: Timer,
    state: State<T>,
    filter_builder: FilterBuilder,
    topic: Vec<H256>,
    chain: ChainAlias,
}

impl<T: Transport> LogStream<T> {
    pub fn new(options: LogStreamOptions<T>) -> Self {
        let timer = Timer::default();

        let topic = ethabi_topic_to_web3(&options.filter.topic0)
            .expect("filter must have at least 1 topic. q.e.d.");
        let filter_builder = filter_to_builder(&options.filter, options.contract_address);

        let block_number_stream_options = BlockNumberStreamOptions {
            request_timeout: options.request_timeout,
            poll_interval: options.poll_interval,
            confirmations: options.confirmations,
            transport: options.transport.clone(),
            last_block_number: options.last_block_number,
        };

        LogStream {
            block_number_stream: BlockNumberStream::new(block_number_stream_options),
            request_timeout: options.request_timeout,
            transport: options.transport,
            last_checked_block: options.last_block_number,
            timer,
            state: State::AwaitBlockNumber,
            filter_builder,
            topic,
            chain: options.chain,
        }
    }
}

impl<T: Transport> Stream for LogStream<T> {
    type Item = LogsInBlockRange;
    type Error = error::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            let (next_state, value_to_yield) = match self.state {
                State::AwaitBlockNumber => {
                    let last_block =
                        try_stream!(self.block_number_stream.poll().chain_err(|| {
                            "LogStream: fetching of last confirmed block number failed"
                        },));
                    debug!("LogStream: fetched confirmed block number {}", last_block);

                    let from = self.last_checked_block + 1;
                    let filter = self
                        .filter_builder
                        .clone()
                        .from_block(from.into())
                        .to_block(last_block.into())
                        .build();
                    let future = match self.chain {
                        ChainAlias::ETH => web3::api::Eth::new(&self.transport).logs(filter),
                        ChainAlias::ABOS => abos_logs(&self.transport, filter),
                    };
                    debug!(
                        "LogStream: fetching logs in blocks {} to {}",
                        from, last_block
                    );

                    let next_state = State::AwaitLogs {
                        from: from,
                        to: last_block,
                        future: self.timer.timeout(future.from_err(), self.request_timeout),
                    };

                    (next_state, None)
                }
                State::AwaitLogs {
                    ref mut future,
                    from,
                    to,
                } => {
                    let logs = try_ready!(future
                        .poll()
                        .chain_err(|| "LogStream: polling web3 logs failed",));
                    info!(
                        "LogStream (topic: {:?}): fetched {} logs from block {} to block {}",
                        self.topic,
                        logs.len(),
                        from,
                        to
                    );
                    let log_range_to_yield = LogsInBlockRange { from, to, logs };

                    self.last_checked_block = to;
                    (State::AwaitBlockNumber, Some(log_range_to_yield))
                }
            };

            self.state = next_state;

            if value_to_yield.is_some() {
                return Ok(Async::Ready(value_to_yield));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts;
    use rustc_hex::FromHex;
    use tokio_core::reactor::Core;
    use web3::types::{Bytes, Log};

    #[test]
    fn test_log_stream_twice_no_logs() {
        let deposit_topic = contracts::bridge::events::ingress::filter().topic0;

        let transport = mock_transport!(
            "eth_blockNumber" =>
                req => json!([]),
                res => json!("0x1011");
            "eth_getLogs" =>
                req => json!([{
                    "address": "0x0000000000000000000000000000000000000001",
                    "fromBlock": "0x4",
                    "toBlock": "0x1005",
                    "topics": [deposit_topic]
                }]),
                res => json!([]);
            "eth_blockNumber" =>
                req => json!([]),
                res => json!("0x1012");
            "eth_getLogs" =>
                req => json!([{
                    "address": "0x0000000000000000000000000000000000000001",
                    "fromBlock": "0x1006",
                    "toBlock": "0x1006",
                    "topics": [deposit_topic]
                }]),
                res => json!([]);
        );

        let log_stream = LogStream::new(LogStreamOptions {
            request_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_secs(1),
            confirmations: 12,
            transport: transport.clone(),
            contract_address: "0000000000000000000000000000000000000001".into(),
            last_block_number: 3,
            filter: contracts::bridge::events::ingress::filter(),
        });

        let mut event_loop = Core::new().unwrap();
        let log_ranges = event_loop.run(log_stream.take(2).collect()).unwrap();

        assert_eq!(
            log_ranges,
            vec![
                LogsInBlockRange {
                    from: 4,
                    to: 4101,
                    logs: vec![],
                },
                LogsInBlockRange {
                    from: 4102,
                    to: 4102,
                    logs: vec![],
                },
            ]
        );
        assert_eq!(transport.actual_requests(), transport.expected_requests());
    }

    #[test]
    fn test_log_stream_once_one_log() {
        let deposit_topic = contracts::bridge::events::ingress::filter().topic0;

        let transport = mock_transport!(
            "eth_blockNumber" =>
                req => json!([]),
                res => json!("0x1011");
            "eth_getLogs" =>
                req => json!([{
                    "address": "0x0000000000000000000000000000000000000001",
                    "fromBlock": "0x4",
                    "toBlock": "0x1005",
                    "topics": [deposit_topic],
                }]),
                res => json!([{
                    "address": "0x0000000000000000000000000000000000000cc1",
                    "topics": [deposit_topic],
                    "data": "0x000000000000000000000000aff3454fce5edbc8cca8697c15331677e6ebcccc00000000000000000000000000000000000000000000000000000000000000f0",
                    "type": "",
                    "transactionHash": "0x884edad9ce6fa2440d8a54cc123490eb96d2768479d49ff9c7366125a9424364"
                }]);
        );

        let log_stream = LogStream::new(LogStreamOptions {
            request_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_secs(1),
            confirmations: 12,
            transport: transport.clone(),
            contract_address: "0000000000000000000000000000000000000001".into(),
            last_block_number: 3,
            filter: contracts::bridge::events::ingress::filter(),
        });

        let mut event_loop = Core::new().unwrap();
        let log_ranges = event_loop.run(log_stream.take(1).collect()).unwrap();

        assert_eq!(
            log_ranges,
            vec![
                LogsInBlockRange { from: 4, to: 4101, logs: vec![
                    Log {
                        address: "0x0000000000000000000000000000000000000cc1".into(),
                        topics: deposit_topic.into(),
                        data: Bytes("000000000000000000000000aff3454fce5edbc8cca8697c15331677e6ebcccc00000000000000000000000000000000000000000000000000000000000000f0".from_hex().unwrap()),
                        transaction_hash: Some("0x884edad9ce6fa2440d8a54cc123490eb96d2768479d49ff9c7366125a9424364".into()),
                        block_hash: None,
                        block_number: None,
                        transaction_index: None,
                        log_index: None,
                        transaction_log_index: None,
                        log_type: None,
                        removed: None,
                    }
                ] },
            ]);
        assert_eq!(transport.actual_requests(), transport.expected_requests());
    }
}
