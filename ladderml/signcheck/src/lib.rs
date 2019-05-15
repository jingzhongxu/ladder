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

pub trait Trait: system::Trait {
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
        Txisok(Hash),
        // 交易 = vec<id，签名>
        TranscationVerified(Hash,Vec<(AccountId,Hash)>),
    }
);

decl_storage! {
    trait Store for Module<T: Trait> as Signature {

        /// 记录每个交易的签名的数量
        /// Record the number of signatures per transaction got
        NumberOfSignedContract get(num_of_signed): map T::Hash => u64;

        /// 需要这些数量的签名，才发送这个交易通过的事件
        /// These amount of signatures are needed to send the event that the transaction verified.
        MinNumOfSignature get(min_signature): u64;

        //record transaction   Hash => (accountid,sign)
        IdSignTxList  get(all_list) : map T::Hash => (T::AccountId,T::Hash);
        Record  get(record) : map T::Hash => Vec<(T::AccountId,T::Hash)>;
       // IdSignTxListC  get(all_list_c) : map T::Hash => Vec<T::AccountId>;

        /// 是否有重复的签名   map 交易Hash => 签名Hash列表
        RepeatPrevent  get(repeat_prevent) : map T::Hash => Vec<T::Hash>;

        /// 已经发送过的交易记录  防止重复发送事件
        /// Transaction records that have been sent prevent duplication of events
        AlreadySentTx get(already_sent) : map T::Hash => u64;

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
 /*
        /// 签名并判断如果当前签名数量足够就发送一个事件
        /// Sign and determine if the current number of signatures is sufficient to send an event
        pub  fn sign_and_check(origin,transcation : T::Hash,sign : T::Hash) -> Result{
            let sender = ensure_signed(origin)?;
            //判断是否是合法验证者集合中的人
            let validators = <session::Module<T>>::validators();
            ensure!(!validators.contains(&sender),"not validator");

            //查看该交易是否存在，没得话添加上去
            if !<NumberOfSignedContract<T>>::exists(transcation) {
                <NumberOfSignedContract<T>>::insert(&transcation,0);
                <AlreadySentTx<T>>::insert(&transcation,0);
            }

            //查看这个签名的是否重复发送交易 重复发送就滚粗
            let mut repeat_vec = Self::all_list_c(transcation);
            ensure!(repeat_vec.contains(&sender),"repeat!");

            //查看交易是否已被发送
            if 1 == Self::already_sent(transcation){
                 return Err("has been sent");
            }

/*    这个记录好像没用
            //增加一条记录 ->  交易 验证者 签名
            <IdSignTxList<T>>::insert(transcation.clone(),(sender.clone(),sign.clone()));
*/

            //增加一条记录 ->  交易 = vec of 验证者 签名
            let mut stored_vec = Self::all_list_b(transcation);
            stored_vec.push((sender.clone(),sign.clone()));
            <IdSignTxListB<T>>::insert(transcation.clone(),stored_vec.clone());
            repeat_vec.push(sender.clone());
            <IdSignTxListC<T>>::insert(transcation.clone(),repeat_vec.clone());

            //其他验证？
            Self::_verify(transcation)?;

            let numofsigned = Self::num_of_signed(&transcation);
            let newnumofsigned = numofsigned.checked_add(1)
            .ok_or("Overflow adding a new sign to Tx")?;

            <NumberOfSignedContract<T>>::insert(&transcation,newnumofsigned);
            if newnumofsigned <= Self::min_signature() {
                return Err("not enough signatusign_and_checkre");
            }


            // Record the transaction and sending event
            <AlreadySentTx<T>>::insert(&transcation,1);
            Self::deposit_event(RawEvent::Txisok(transcation));

            Self::deposit_event(RawEvent::TranscationVerified(transcation,stored_vec));
            Ok(())
        }
     */
    }
}

impl<T: Trait> Module<T> {
    fn _verify(_tx: T::Hash) -> Result {
        //TODO:verify signature or others
        Ok(())
    }

    /// 签名并判断如果当前签名数量足够就发送一个事件
    /// Sign and determine if the current number of signatures is sufficient to send an event
    pub fn check_signature(
        who: T::AccountId,
        transcation: T::Hash,
        sign: T::Hash,
        message: T::Hash,
    ) -> Result {
        //TODO： 判断这个信息发送的人是否是validator     不在这里 已经前置了
        let sender = who;

        //查看该交易是否已经存在，没得话添加上去
        if !<NumberOfSignedContract<T>>::exists(transcation) {
            <NumberOfSignedContract<T>>::insert(&transcation, 0);
            <AlreadySentTx<T>>::insert(&transcation, 0);
        }

        /// 防止签名重复
        // 防止一个交易的相同签名的是否重复发送
        let mut repeat_vec = Self::repeat_prevent(transcation);
        ensure!(!repeat_vec.contains(&sign), "This signature is repeat!");
        // ensure!(!repeat_vec.iter().find(|&t| t == &sign).is_none(), "Cannot deposit if already in queue.");
        // 把某个交易hash的一个签名hash保存，以验证后来的是否重复
        repeat_vec.push(sign.clone());
        <RepeatPrevent<T>>::insert(transcation.clone(), repeat_vec.clone());

        // 防止一个交易被重复发送，已发送过的会有记录
        if 1 == Self::already_sent(transcation) {
            return Err("This Transcation already been sent!");
        }

        /*
             //增加一条记录 ->  交易 验证者 签名
             <IdSignTxList<T>>::insert(transcation.clone(),(sender.clone(),sign.clone()));
        */

        //增加一条记录  包含  交易hash => vec (验证者,签名hash)
        let mut stored_vec = Self::record(transcation);
        stored_vec.push((sender.clone(), sign.clone()));
        <Record<T>>::insert(transcation.clone(), stored_vec.clone());

        //TODO:其他验证
        Self::_verify(transcation)?;

        // 判断签名数量是否达到指定要求
        let numofsigned = Self::num_of_signed(&transcation);
        let newnumofsigned = numofsigned
            .checked_add(1)
            .ok_or("Overflow adding a new sign to Tx")?;

        <NumberOfSignedContract<T>>::insert(&transcation, newnumofsigned);
        if newnumofsigned <= Self::min_signature() {
            return Err("Not enough signature!");
        }

        // 记录已发送的交易防止重复发送 Record the transaction and sending event
        <AlreadySentTx<T>>::insert(&transcation, 1);

        // 抛出事件
        Self::deposit_event(RawEvent::Txisok(transcation));
        Self::deposit_event(RawEvent::TranscationVerified(transcation, stored_vec));
        Ok(())
    }
}
