#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use serde_derive::{Deserialize, Serialize};

use sr_primitives::traits::{CheckedAdd, CheckedSub, Hash, Verify, Zero};
use support::{
    decl_event, decl_module, decl_storage, dispatch::Result, ensure, Parameter, StorageMap,
    StorageValue,
};

use system::ensure_signed;

use rstd::marker::PhantomData;
use rstd::prelude::*;

#[cfg(feature = "std")]
pub use std::fmt;

// use Encode, Decode
use parity_codec::{Decode, Encode};
use rstd::ops::Div;

pub trait Trait: session::Trait + bank::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_event!(
    pub enum Event<T>
    where
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash
    {
        Created(AccountId, Hash),
        SetMinRequreSignatures(u64),
        Txisok(u64),
        // 交易 = vec<id，签名>
        TranscationVerified(u64,Vec<(AccountId,u64)>),
    }
);

decl_storage! {
    trait Store for Module<T: Trait> as Exchange {

        /// 记录每个交易的签名的数量
        /// Record the number of signatures per transaction got
        NumberOfSignedContract get(num_of_signed): map u64 => u64;

        /// 需要这些数量的签名，才发送这个交易通过的事件
        /// These amount of signatures are needed to send the event that the transaction verified.
        MinNumOfSignature get(min_signature): u64;

        //record transaction   Hash => (accountid,sign)
        IdSignTxList  get(all_list) : map u64 => (T::AccountId,u64);
        IdSignTxListB  get(all_list_b) : map u64 => Vec<(T::AccountId,u64)>;
        RepeatPrevent  get(repeat) : map u64 => Vec<T::AccountId>;

        /// 已经发送过的交易记录  防止重复发送事件
        /// Transaction records that have been sent prevent duplication of events
        AlreadySentTx get(already_sent) : map u64 => u64;

       // Nonce: u64;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        fn deposit_event<T>() = default;

        /// 设置最小要求签名数量
        /// Set the minimum required number of signatures
        pub  fn set_min_num(origin,new_num: u64) -> Result{
            let _sender = ensure_signed(origin)?;
            let newmin = new_num;
            if newmin < 5 {
                return Err("too small,should bigger than 5");
            }
            <MinNumOfSignature<T>>::put(newmin) ;
            Self::deposit_event(RawEvent::SetMinRequreSignatures(newmin));
            Ok(())
        }

        /// 签名并判断如果当前签名数量足够就发送一个事件
        pub  fn check_exchange(origin, message: Vec<u8>, signature: Vec<u8>) -> Result{

        let (who , exchangerate  ,time ,extype) = Self::parse_data(message, signature);
        let sender = who;
        Self::check_signature(sender,exchangerate,time,extype);
        Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    fn _verify(_tx: T::Hash) -> Result {
        //TODO:verify signature or others
        Ok(())
    }

    pub fn check_validator(accountid: &T::AccountId) -> bool {
        //判断是否是合法验证者集合中的人
        let validators = <session::Module<T>>::validators();
        if !validators.contains(accountid) {
            return false;
        }
        true
    }

    /// 签名并判断如果当前签名数量足够就发送一个事件
    /// Sign and determine if the current number of signatures is sufficient to send an event
    pub fn check_signature(who: T::AccountId, exchangerate: u64, time: u64, extype: u64) -> Result {
        let sender = who;

        //查看该交易是否存在，没得话添加上去
        if !<NumberOfSignedContract<T>>::exists(time) {
            <NumberOfSignedContract<T>>::insert(&time, 0);
            <AlreadySentTx<T>>::insert(&time, 0);
        }

        //查看这个签名的是否重复发送交易 重复发送就滚粗
        let mut repeat_vec = Self::repeat(time);
        ensure!(repeat_vec.contains(&sender), "repeat!");

        //查看交易是否已被发送
        if 1 == Self::already_sent(time) {
            return Err("has been sent");
        }

        //增加一条记录 ->  交易 验证者 签名
        <IdSignTxList<T>>::insert(time.clone(), (sender.clone(), exchangerate.clone()));

        //增加一条记录 ->  交易 = vec of 验证者 签名
        let mut stored_vec = Self::all_list_b(time);
        stored_vec.push((sender.clone(), exchangerate.clone()));
        <IdSignTxListB<T>>::insert(time.clone(), stored_vec.clone());
        repeat_vec.push(sender.clone());
        <RepeatPrevent<T>>::insert(time.clone(), repeat_vec.clone());

        //其他验证？
        //Self::_verify(transcation)?;

        let numofsigned = Self::num_of_signed(&time);
        let newnumofsigned = numofsigned
            .checked_add(1)
            .ok_or("Overflow adding a new sign to Tx")?;
        <NumberOfSignedContract<T>>::insert(&time, newnumofsigned);
        if newnumofsigned <= Self::min_signature() {
            return Err("not enough signatusign_and_checkre");
        }

        // Record the transaction and sending event
        <AlreadySentTx<T>>::insert(&time, 1);
        Self::deposit_event(RawEvent::Txisok(time));

        Self::deposit_event(RawEvent::TranscationVerified(time, stored_vec));
        Ok(())
    }

    /// 解析message --> hash tag  id  exchangerate time extype
    ///    bit          32   32   32     8          8    8
    ///    实现         ok   no   ok     ok         ok   ok
    ///           解析出   发送者id   汇率  该汇率时间  汇率类型（保留字段）
    fn parse_data(message: Vec<u8>, signature: Vec<u8>) -> (T::AccountId, u64, u64, u64) {
        let mut messagedrain = message.clone();
        let hash: Vec<_> = messagedrain.drain(0..32).collect();

        let tx_hash: T::Hash = Decode::decode(&mut &hash.encode()[..]).unwrap();

        let signature_hash: T::Hash = Decode::decode(&mut &signature.encode()[..]).unwrap();
        let id: Vec<_> = messagedrain.drain(32..64).collect();
        let who: T::AccountId = Decode::decode(&mut &id.encode()[..]).unwrap();

        let rate_vec: Vec<u8> = messagedrain.drain(32..40).collect();
        let mut rate: u64 = 0;
        let mut i = 0;
        rate_vec.iter().for_each(|x| {
            let exp = (*x as u64) ^ i;
            rate = rate + exp;
            i = i + 1;
        });

        let time_vec: Vec<u8> = messagedrain.drain(32..40).collect();
        let mut time: u64 = 0;
        let mut v = 0;
        time_vec.iter().for_each(|x| {
            let exp = (*x as u64) ^ v;
            time = time + exp;
            v = v + 1;
        });

        let type_vec: Vec<u8> = messagedrain.drain(32..40).collect();
        let mut extype: u64 = 0;
        let mut q = 0;
        type_vec.iter().for_each(|x| {
            let exp = (*x as u64) ^ q;
            extype = extype + exp;
            q = q + 1;
        });
        //ensure the signature is valid
        let mut tx_hash_to_check: [u8; 65] = [0; 65];
        tx_hash_to_check.clone_from_slice(&hash);
        let mut signature_hash_to_check: [u8; 32] = [0; 32];
        signature_hash_to_check.clone_from_slice(&signature);
        <bank::Module<T>>::check_secp512(&tx_hash_to_check, &signature_hash_to_check).is_ok();

        return (who, rate, time, extype);
    }
}
