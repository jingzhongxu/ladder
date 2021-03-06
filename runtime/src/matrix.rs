// extern crate srml_session as session;
// extern crate srml_balances as balances;
// extern crate sr_io as runtime_io;
// extern crate substrate_primitives as primitives;


use session;
use balances;
use rstd::prelude::Vec;
use runtime_primitives::traits::*;
use { system::{self, ensure_signed}};
use support::{
    decl_module, decl_storage, decl_event, StorageMap, dispatch::Result, ensure
};

pub trait Trait: balances::Trait + session::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;

        /// Data Forwarding Request Message
        /// offset  0: 32 bytes :: uint256 - tag
        //  offset 32: 20 bytes :: address - recipient address
        //  offset 52: 32 bytes :: uint256 - value
        //  offset 84: 32 bytes :: bytes32 - transaction hash
        pub fn ingress(origin, message: Vec<u8>, signature: Vec<u8>) -> Result {
            let sender = ensure_signed(origin)?;
            let hash = T::Hashing::hash_of(&message);

            let signature_hash = T::Hashing::hash_of(&signature);
            if let Ok(()) = Self::verify_ingress_message(sender,hash,signature_hash ) {

                Self::deposit_event(RawEvent::Ingress(signature.clone(), message.clone()));
                <IngressOf<T>>::insert(hash, message.clone());
                return  Ok(());
            }
            Err("ingress err")
        }

        /// Data Forwarding Confirmation Message
        pub fn egress(origin, message: Vec<u8>, signature: Vec<u8>) -> Result {
            let sender = ensure_signed(origin)?;
            let hash = T::Hashing::hash_of(&message);
            let signature_hash = T::Hashing::hash_of(&signature);
            if let Ok(()) = Self::verify_egress_message(sender,hash,signature_hash ) {

                Self::deposit_event(RawEvent::Egress(signature.clone(), message.clone()));
                <EgressOf<T>>::insert(hash, message.clone());
                 return  Ok(());
            }
             Err("egress err")
        }

        /// Data Forwarding Timeout Return Message
        pub fn rollback(_origin, _message: Vec<u8>, _signature: Vec<u8>) -> Result {
            Ok(())
        }


        /// Resetting Validation Node Messages
        /// offset 0: 32 bytes :: uint256 - block number
        /// offset 32: 20 bytes :: address - authority0
        /// offset 52: 20 bytes :: address - authority1
        /// offset 72: 20 bytes :: ..    ....
        pub fn reset_authorities(_origin, _message: Vec<u8>, _signature: Vec<u8>) -> Result {
            Ok(())
        }


		}
    }


decl_storage! {
    trait Store for Module<T: Trait> as Matrix {
        //pub IngressOf get(ingress_of): map T::Hash => Vec<u8>;

        /// ingress & egress
        MinNumberOfSignatureLimit get(min_signature_limit): u64;

        //记录每个交易的签名的数量
        NumberOfSignedIngressTx get(number_of_signed_ingress): map T::Hash => u64;
        //已经发送过的交易记录  防止重复发送事件
        AlreadySentIngressTx get(already_sent_ingress) : map T::Hash => u64;
        //已经签名过得人记录一下
        IngressSignedSender  get(ingress_signed_sender) : map T::Hash => Vec<T::AccountId>;
        //保存Ingress的相关内容
        IngressList  get(ingress_list) : map T::Hash => Vec<(T::AccountId,T::Hash)>;
        //
        IngressOf  get(ingress_of) :  map T::Hash  => Vec<u8> ;

        //记录每个交易的签名的数量
        NumberOfSignedEgressTx get(number_of_signed_egress): map T::Hash => u64;
        //已经发送过的交易记录  防止重复发送事件
        AlreadySentEgressTx get(already_sent_egress) : map T::Hash => u64;
        //已经签名过得人记录一下
        EgressSignedSender  get(egress_signed_sender) : map T::Hash => Vec<T::AccountId>;
        //保存Egress的相关内容
        EgressList  get(egress_list) : map T::Hash => Vec<(T::AccountId,T::Hash)>;
        //
        EgressOf  get(egress_of) : map T::Hash => Vec<u8>;


    }
}


decl_event! {
    pub enum Event<T> where
        <T as balances::Trait>::Balance,
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash,
        <T as system::Trait>::BlockNumber
    {
        Ingress(Vec<u8>, Vec<u8>),
        Egress(Vec<u8>, Vec<u8>),

          // 交易 = vec<id，签名>
        IngressVerified(Hash,Vec<(AccountId,Hash)>),
        EgressVerified(Hash,Vec<(AccountId,Hash)>),

        Has(Hash),

        //bank moduel
		/// All validators have been rewarded by the given balance.
		Reward(Balance),
		//
		AddDepositingQueue(AccountId),

        NewRewardSession(BlockNumber),

    } 
}

impl<T: Trait> Module<T>
{

    /// 数据转发请求消息 ingress
    /// Data Forwarding Confirmation Message
    fn verify_ingress_message(sender: T::AccountId, message: T::Hash, signature: T::Hash) -> Result{

        //是否在验证者集合中
        let validator_set = <session::Module<T>>::validators();
        ensure!(!validator_set.contains(&sender),"not validator");

        //查看该交易是否存在，没得话添加上去
        if !<NumberOfSignedIngressTx<T>>::exists(message) {
            <NumberOfSignedIngressTx<T>>::insert(&message,0);
            <AlreadySentIngressTx<T>>::insert(&message,0);
        }

        //查看这个签名的是否重复发送交易 重复发送就
        let mut repeat_vec = Self::ingress_signed_sender(&message);
        ensure!(repeat_vec.contains(&sender),"repeat!");

        //查看交易是否已被发送
        if 1 == Self::already_sent_ingress(&message){
            return Err("has been sent");
        }

        //增加一条记录 ->  交易 = vec of 验证者 签名
        let mut stored_vec = Self::ingress_list(&message);
        stored_vec.push((sender.clone(), signature.clone()));
        <IngressList<T>>::insert(message.clone(), stored_vec.clone());
        //更新重复记录
        repeat_vec.push(sender.clone());
        <IngressSignedSender<T>>::insert(message.clone(),repeat_vec.clone());

        //记录已经发送过的交易  同时发送事件Event
        <AlreadySentIngressTx<T>>::insert(&message,1);
        Self::deposit_event(RawEvent::IngressVerified(message,stored_vec));
        Ok(())
    }

    /// 数据转发确认消息 egress
    /// Data Forwarding Confirmation Message
    fn verify_egress_message(sender: T::AccountId, message: T::Hash, signature: T::Hash) -> Result{
        //是否在验证者集合中
        let validator_set = <session::Module<T>>::validators();
        ensure!(!validator_set.contains(&sender),"not validator");

        //查看该交易是否存在，没得话添加上去
        if !<NumberOfSignedEgressTx<T>>::exists(message) {
            <NumberOfSignedEgressTx<T>>::insert(&message,0);
            <AlreadySentEgressTx<T>>::insert(&message,0);
        }

        //查看这个签名的是否重复发送交易 重复发送就滚粗
        let mut repeat_vec = Self::egress_signed_sender(&message);
        ensure!(repeat_vec.contains(&sender),"repeat!");

        //查看交易是否已被发送
        if 1 == Self::already_sent_egress(&message){
            return Err("has been sent");
        }

        //增加一条记录 ->  交易 = vec of 验证者 签名
        let mut stored_vec = Self::egress_list(&message);
        stored_vec.push((sender.clone(), signature.clone()));
        <EgressList<T>>::insert(message.clone(), stored_vec.clone());
        //更新重复记录
        repeat_vec.push(sender.clone());
        <EgressSignedSender<T>>::insert(message.clone(),repeat_vec.clone());

        //记录已经发送过的交易  同时发送事件Event
        <AlreadySentEgressTx<T>>::insert(&message,1);
        Self::deposit_event(RawEvent::EgressVerified(message,stored_vec));
        Ok(())
    }
}