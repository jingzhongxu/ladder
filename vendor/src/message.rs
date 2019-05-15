use crate::events::*;
use web3::types::H256;

#[derive(Debug)]
pub enum RelayType {
    Ingress,
    Egress,
    Deposit,
    Withdraw,
    SetAuthorities,
    ExchangeRate,
}

#[derive(Debug)]
pub struct RelayMessage {
    /// The hash of transaction.
    pub hash: H256,
    /// The raw data of log.
    pub raw: Vec<u8>,
    /// The type of Message.
    pub ty: RelayType,
}

impl From<IngressEvent> for RelayMessage {
    fn from(event: IngressEvent) -> Self {
        RelayMessage {
            hash: event.tx_hash,
            raw: event.to_bytes(),
            ty: RelayType::Ingress,
        }
    }
}

impl From<EgressEvent> for RelayMessage {
    fn from(event: EgressEvent) -> Self {
        RelayMessage {
            hash: event.tx_hash,
            raw: event.to_bytes(),
            ty: RelayType::Egress,
        }
    }
}

impl From<DepositEvent> for RelayMessage {
    fn from(event: DepositEvent) -> Self {
        RelayMessage {
            hash: event.tx_hash,
            raw: event.to_bytes(),
            ty: RelayType::Deposit,
        }
    }
}

impl From<WithdrawEvent> for RelayMessage {
    fn from(event: WithdrawEvent) -> Self {
        RelayMessage {
            hash: event.tx_hash,
            raw: event.to_bytes(),
            ty: RelayType::Withdraw,
        }
    }
}

impl From<AuthorityEvent> for RelayMessage {
    fn from(event: AuthorityEvent) -> Self {
        RelayMessage {
            hash: event.tx_hash,
            raw: event.to_bytes(),
            ty: RelayType::SetAuthorities,
        }
    }
}

impl From<ExchangeRateEvent> for RelayMessage {
    fn from(event: ExchangeRateEvent) -> Self {
        RelayMessage {
            hash: event.tx_hash,
            raw: event.to_bytes(),
            ty: RelayType::ExchangeRate,
        }
    }
}
