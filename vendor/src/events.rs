use crate::error::Error;
use crate::utils::IntoRawLog;
use contracts;
use std::str::FromStr;
use web3::types::{Address, Log, H256, U256};

pub const ETH_COIN: &str = "0000000000000000000000000000000000000000000000000000000000000001";
pub const MESSAGE_LENGTH: usize = 116;
pub const BANKER_LENGTH: usize = 116;
pub const AUTHORITY_MINIMUM_LENGTH: usize = 72;
pub const ORACLE_LENTH: usize = 116; // 8 8

#[derive(Debug)]
pub struct IngressEvent {
    pub tag: H256,
    pub recipient: Address,
    pub value: U256,
    pub tx_hash: H256,
}

impl IngressEvent {
    pub fn from_log(raw_log: &Log) -> Result<Self, Error> {
        let hash = raw_log
            .transaction_hash
            .ok_or_else(|| "`log` must be mined and contain `transaction_hash`")?;
        let log = contracts::bridge::events::ingress::parse_log(raw_log.into_raw_log())?;
        Ok(Self {
            tag: log.tag,
            recipient: log.recipient,
            value: log.value,
            tx_hash: hash,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != MESSAGE_LENGTH {
            bail!("`bytes`.len() must be {}", MESSAGE_LENGTH);
        }

        Ok(Self {
            tag: bytes[0..32].into(),
            recipient: bytes[32..52].into(),
            value: U256::from_big_endian(&bytes[52..84]),
            tx_hash: bytes[84..MESSAGE_LENGTH].into(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = vec![0u8; MESSAGE_LENGTH];
        result[0..32].copy_from_slice(&self.tag.0[..]);
        result[32..52].copy_from_slice(&self.recipient.0[..]);
        self.value.to_big_endian(&mut result[52..84]);
        result[84..MESSAGE_LENGTH].copy_from_slice(&self.tx_hash.0[..]);
        return result;
    }
}

#[derive(Debug)]
pub struct EgressEvent {
    pub tag: H256,
    pub recipient: Address,
    pub value: U256,
    pub tx_hash: H256,
}

impl EgressEvent {
    pub fn from_log(raw_log: &Log) -> Result<Self, Error> {
        let hash = raw_log
            .transaction_hash
            .ok_or_else(|| "`log` must be mined and contain `transaction_hash`")?;
        let log = contracts::bridge::events::egress::parse_log(raw_log.into_raw_log())?;
        Ok(Self {
            tag: log.tag,
            recipient: log.recipient,
            value: log.value,
            tx_hash: hash,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != MESSAGE_LENGTH {
            bail!("`bytes`.len() must be {}", MESSAGE_LENGTH);
        }

        Ok(Self {
            tag: bytes[0..32].into(),
            recipient: bytes[32..52].into(),
            value: U256::from_big_endian(&bytes[52..84]),
            tx_hash: bytes[84..MESSAGE_LENGTH].into(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = vec![0u8; MESSAGE_LENGTH];
        result[0..32].copy_from_slice(&self.tag.0[..]);
        result[32..52].copy_from_slice(&self.recipient.0[..]);
        self.value.to_big_endian(&mut result[52..84]);
        result[84..MESSAGE_LENGTH].copy_from_slice(&self.tx_hash.0[..]);
        return result;
    }
}

#[derive(Debug)]
pub struct DepositEvent {
    pub coin: H256,
    pub recipient: Address,
    pub value: U256,
    pub tx_hash: H256,
}

impl DepositEvent {
    pub fn from_log(raw_log: &Log) -> Result<Self, Error> {
        let hash = raw_log
            .transaction_hash
            .ok_or_else(|| "`log` must be mined and contain `transaction_hash`")?;
        let log = contracts::bridge::events::deposit::parse_log(raw_log.into_raw_log())?;
        Ok(Self {
            coin: H256::from_str(ETH_COIN).unwrap(),
            recipient: log.beneficiary,
            value: log.amount,
            tx_hash: hash,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != BANKER_LENGTH {
            bail!("`bytes`.len() must be {}", BANKER_LENGTH);
        }

        Ok(Self {
            coin: bytes[0..32].into(),
            recipient: bytes[32..52].into(),
            value: U256::from_big_endian(&bytes[52..84]),
            tx_hash: bytes[84..BANKER_LENGTH].into(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = vec![0u8; BANKER_LENGTH];
        result[0..32].copy_from_slice(&self.coin.0[..]);
        result[32..52].copy_from_slice(&self.recipient.0[..]);
        self.value.to_big_endian(&mut result[52..84]);
        result[84..BANKER_LENGTH].copy_from_slice(&self.tx_hash.0[..]);
        return result;
    }
}

#[derive(Debug)]
pub struct WithdrawEvent {
    pub coin: H256,
    pub recipient: Address,
    pub value: U256,
    pub tx_hash: H256,
}

impl WithdrawEvent {
    pub fn from_log(raw_log: &Log) -> Result<Self, Error> {
        let hash = raw_log
            .transaction_hash
            .ok_or_else(|| "`log` must be mined and contain `transaction_hash`")?;
        let log = contracts::bridge::events::withdraw::parse_log(raw_log.into_raw_log())?;
        Ok(Self {
            coin: H256::from_str(ETH_COIN).unwrap(),
            recipient: log.beneficiary,
            value: log.amount,
            tx_hash: hash,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != BANKER_LENGTH {
            bail!("`bytes`.len() must be {}", BANKER_LENGTH);
        }

        Ok(Self {
            coin: bytes[0..32].into(),
            recipient: bytes[32..52].into(),
            value: U256::from_big_endian(&bytes[52..84]),
            tx_hash: bytes[84..BANKER_LENGTH].into(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = vec![0u8; BANKER_LENGTH];
        result[0..32].copy_from_slice(&self.coin.0[..]);
        result[32..52].copy_from_slice(&self.recipient.0[..]);
        self.value.to_big_endian(&mut result[52..84]);
        result[84..BANKER_LENGTH].copy_from_slice(&self.tx_hash.0[..]);
        return result;
    }
}

pub fn array_to_u32(arr: [u8; 4]) -> u32 {
    let u = unsafe { std::mem::transmute::<[u8; 4], u32>(arr) };
    u
}

pub fn u32_to_array(data: u32) -> [u8; 4] {
    let bytes: [u8; 4] = unsafe { std::mem::transmute(data.to_le()) };
    bytes
}

#[derive(Debug)]
pub struct AuthorityEvent {
    pub coin: H256,
    pub last_len: u32,
    pub last: Vec<Address>,
    pub next_len: u32,
    pub next: Vec<Address>,
    pub tx_hash: H256,
}

impl AuthorityEvent {
    pub fn from_log(raw_log: &Log) -> Result<Self, Error> {
        let hash = raw_log
            .transaction_hash
            .ok_or_else(|| "`log` must be mined and contain `transaction_hash`")?;
        let log = contracts::bridge::events::replace_auths::parse_log(raw_log.into_raw_log())?;
        Ok(Self {
            coin: H256::from_str(ETH_COIN).unwrap(),
            last_len: (log.last.len() as u32),
            last: log.last,
            next_len: (log.next.len() as u32),
            next: log.next,
            tx_hash: hash,
        })
    }

    /*
    0:  32               bytes  H256  coin
    32: 36               bytes  u32   last_len
    36: 36+20*last_len   bytes  [Address]
    a: a+4               bytes  u32   next_len
    a+4: a+4+20*next_len bytes  [address]
    b: b+32              bytes H256   tx_hash

    min = 32 + 4 + 4 + 32 = 72;
    */
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < AUTHORITY_MINIMUM_LENGTH {
            bail!(
                "`bytes`.len() must be more than {}",
                AUTHORITY_MINIMUM_LENGTH
            );
        }
        let mut index: usize = 0;
        let coin: H256 = bytes[index..(index + 32)].into();
        index += 32;

        let mut tmp: [u8; 4] = [0u8; 4];
        tmp.copy_from_slice(&bytes[index..(index + 4)]);
        let last_len = array_to_u32(tmp);
        index += 4;

        let mut last: Vec<Address> = Vec::with_capacity(last_len as usize);
        for _ in (0..last_len) {
            let address: Address = bytes[index..(index + 20)].into();
            index += 20;
            last.push(address);
        }

        tmp.copy_from_slice(&bytes[index..(index + 4)]);
        let next_len = array_to_u32(tmp);
        index += 4;

        let next: Vec<Address> = (0..next_len)
            .map(|i| {
                let address: Address = bytes[index..(index + 20)].into();
                index += 20;
                address
            })
            .collect();

        let tx_hash = bytes[index..(index + 32)].into();
        Ok(Self {
            coin: coin,
            last_len: last_len,
            last: last,
            next_len: next_len,
            next: next,
            tx_hash: tx_hash,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let capacity = AUTHORITY_MINIMUM_LENGTH
            + (self.last_len as usize) * 20
            + (self.next_len as usize) * 20;
        let mut result: Vec<u8> = Vec::with_capacity(capacity);
        let mut index = 0;
        result[index..(index + 32)].copy_from_slice(&self.coin.0[..]);
        index += 32;
        result[index..(index + 4)].copy_from_slice(&u32_to_array(self.last_len));
        index += 4;
        for i in 0..self.last_len as usize {
            result[index..(index + 20)].copy_from_slice(&(self.last[i].0[..]));
            index += 20;
        }
        result[index..(index + 4)].copy_from_slice(&u32_to_array(self.next_len));
        index += 4;
        for i in 0..self.next_len as usize {
            result[index..(index + 20)].copy_from_slice(&self.next[i].0[..]);
            index += 20;
        }
        result[index..(index + 32)].copy_from_slice(&self.tx_hash.0[..]);
        return result;
    }
}

#[derive(Debug)]
pub struct ExchangeRateEvent {
    pub pair: u64, //组合类型 1-ETHUSD  2-BITUSD  ……
    pub time: u64, //该汇率的时间
    pub rate: u64,
    pub tx_hash: H256,
}

impl ExchangeRateEvent {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = vec![0u8; 24];
        result[0..8].copy_from_slice(&u64_to_array(self.rate));
        result[8..16].copy_from_slice(&u64_to_array(self.time));
        result[16..24].copy_from_slice(&u64_to_array(self.pair));

        result[24..56].copy_from_slice(&self.tx_hash.0[..]);
        return result;
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != 56 {
            bail!("`bytes`.len() must be {}", 56);
        }
        let mut tmp: [u8; 8] = [0u8; 8];

        let raw_pair = tmp.copy_from_slice(&bytes[0..8]);
        let pair = array_to_u64(tmp);

        let raw_time = tmp.copy_from_slice(&bytes[8..16]);
        let time = array_to_u64(tmp);

        let raw_rate = tmp.copy_from_slice(&bytes[16..24]);
        let rate = array_to_u64(tmp);

        Ok(Self {
            pair: pair,
            time: time,
            rate: rate,
            tx_hash: bytes[24..56].into(),
        })
    }
}
pub fn u64_to_array(data: u64) -> [u8; 8] {
    let bytes: [u8; 8] = unsafe { std::mem::transmute(data.to_le()) };
    bytes
}

pub fn array_to_u64(arr: [u8; 8]) -> u64 {
    let u = unsafe { std::mem::transmute::<[u8; 8], u64>(arr) };
    u
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hex::{FromHex, ToHex};
    use web3::types::Bytes;

    fn prepare_data() -> (H256, Address, U256, H256, &'static str) {
        let tag: H256 = "0x0000000000000000000000000000000000000000000000000000000000000002".into();
        let recipient: Address = "0x74241db5f3ebaeecf9506e4ae988186093341604".into();
        // 0x00000000000000000000000000000000000000000000000000000000054c5638
        let value: U256 = U256::from_dec_str("88888888").unwrap();
        let tx_hash: H256 =
            "0x1045bfe274b88120a6b1e5d01b5ec00ab5d01098346e90e7c7a3c9b8f0181c80".into();
        let bytes_str: &'static str = "000000000000000000000000000000000000000000000000000000000000000274241db5f3ebaeecf9506e4ae98818609334160400000000000000000000000000000000000000000000000000000000054c56381045bfe274b88120a6b1e5d01b5ec00ab5d01098346e90e7c7a3c9b8f0181c80";
        (tag, recipient, value, tx_hash, bytes_str)
    }

    #[test]
    fn test_message_to_bytes() {
        let (tag, recipient, value, tx_hash, bytes_str) = prepare_data();

        let message = IngressEvent {
            tag: tag,
            recipient: recipient,
            value: value,
            tx_hash: tx_hash,
        };
        println!("{}", message.to_bytes().to_hex());
        assert_eq!(message.to_bytes().to_hex(), bytes_str);
    }

    #[test]
    fn test_message_from_bytes() {
        let (tag, recipient, value, tx_hash, bytes_str) = prepare_data();

        let message = IngressEvent::from_bytes(bytes_str.from_hex().unwrap().as_slice()).unwrap();
        assert_eq!(message.tag, tag);
        assert_eq!(message.recipient, recipient);
        assert_eq!(message.value, value);
        assert_eq!(message.tx_hash, tx_hash);
    }

    #[test]
    fn test_message_from_log() {
        let (tag, recipient, value, tx_hash, bytes_str) = prepare_data();
        let ingress_topic = contracts::bridge::events::ingress::filter().topic0;
        let log = Log {
                    address: "0xf1dF5972B7e394201d4fFADD797FAa4A3C8be0ea".into(),
                    topics: ingress_topic.into(),
                    data: Bytes("000000000000000000000000000000000000000000000000000000000000000200000000000000000000000074241db5f3ebaeecf9506e4ae98818609334160400000000000000000000000074241db5f3ebaeecf9506e4ae98818609334160400000000000000000000000000000000000000000000000000000000054c5638".from_hex().unwrap()),
                    transaction_hash: Some(tx_hash),
                    block_hash: None,
                    block_number: None,
                    transaction_index: None,
                    log_index: None,
                    transaction_log_index: None,
                    log_type: None,
                    removed: None,
                };
        let message = IngressEvent::from_log(&log).unwrap();
        assert_eq!(message.tag, tag);
        assert_eq!(message.recipient, recipient);
        assert_eq!(message.value, value);
    }
}
