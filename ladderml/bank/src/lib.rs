#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use serde_derive::{Deserialize, Serialize};

use sr_primitives::traits::{As, CheckedAdd, CheckedSub, Hash, One, Verify, Zero};
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

use support::traits::Currency;

/*
/// 用来存储奖励转换算法
#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RewardFactor<U> {
    number: U,
    x: Vec<U>,
    y: Vec<U>,
}
*/
type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: balances::Trait + session::Trait + signcheck::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    type Currency: Currency<Self::AccountId>;
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
    //pub struct Module<T: Trait<I>, I: Instance = DefaultInstance> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;

        /// deposit
        //offset 0:32  who         ::   AccountID
        //offset 32:48 bytes        ::   money
        //origin, hash 32: T::Hash, tag 32: T::Hash, id 20: T::AccountId, amount 32: T::Balance, signature: Vec<u8>
        //(origin, message: Vec, signature: Vec)
        pub fn deposit(origin, message: Vec<u8>, signature: Vec<u8>) -> Result {
            let sender = ensure_signed(origin)?;

            //TODO: 在这里判断 sender 是否有权限提交
            let validators = <session::Module<T>>::validators();
            ensure!(validators.contains(&sender),"Not validator");

            // 解析message --> 以太坊交易的hash tx_hash  abmatrix上的账号who
            //                 该账号的抵押数量amount   整个交易的签名signature_hash
            let (_tx_hash, who, amount, signature_hash) = Self::split_message(message.clone(),signature);
            // 整个交易的hash
            let message_hash = Decode::decode(&mut &message.encode()[..]).unwrap();

            runtime_io::print("开始判断是否重复抵押");
            // ensure no repeat desposit
            ensure!(Self::despositing_account().iter().find(|&t| t == &who).is_none(), "Cannot deposit if already depositing.");
            // ensure no repeat intentions to desposit
            ensure!(Self::intentions_desposit_vec().iter().find(|&t| t == &who).is_none(), "Cannot deposit if already in queue.");

            //check the validity and number of signatures
            runtime_io::print("开始检查签名");
            match  Self::check_signature(sender.clone(), message_hash, signature_hash, message_hash){
                Ok(y) =>  runtime_io::print("ok") ,
                Err(x) => return Err(x),
            }

            // update the list of intentions to desposit
            runtime_io::print("抵押账号通过验证=>存储其 accountid 和 balance 入intentions");
            // update the list of intentions to desposit
            <IntentionsDespositVec<T>>::put({
                let mut v =  Self::intentions_desposit_vec();
                v.push(who.clone());
                v
            });
            <IntentionsDesposit<T>>::insert(who.clone(),T::Balance::sa(amount as u64));
            // 发送一个event
            Self::deposit_event(RawEvent::AddDepositingQueue(who));
            Ok(())
        }

        /// 直接传参数抵押测试用接口
        pub fn deposit2  (origin, hash: T::Hash, _tag: T::Hash, id: T::AccountId,amount: T::Balance, signature: Vec<u8>) -> Result {
            let sender = ensure_signed(origin)?;
            let who =  id;

            let validators = <session::Module<T>>::validators();
            ensure!(validators.contains(&sender),"Not validator");

            // ensure no repeat
            ensure!(Self::despositing_account().iter().find(|&t| t == &who).is_none(), "Cannot deposit if already depositing.");
            // ensure no repeat
            ensure!(Self::intentions_desposit_vec().iter().find(|&t| t == &who).is_none(), "Cannot deposit if already in queue.");

            //decode the signature
            let signature_hash =  Decode::decode(&mut &signature.encode()[..]).unwrap();

            runtime_io::print("开始检查签名");
            match  Self::check_signature(sender.clone(), hash, signature_hash, hash){
                Ok(y) =>  runtime_io::print("ok") ,
                Err(x) => return Err(x),
            }

            runtime_io::print("抵押账号通过验证=>存储其 accountid 和 balance 入intentions");
            // update the list of intentions to desposit
            <IntentionsDespositVec<T>>::put({
                let mut v =  Self::intentions_desposit_vec();
                v.push(who.clone());
                v
            });

            <IntentionsDesposit<T>>::insert(who.clone(),amount);

            // 发送一个event
            Self::deposit_event(RawEvent::AddDepositingQueue(who));
            Ok(())
        }

        // 领取500快
        pub fn get_free_money(origin, who: T::AccountId) -> Result {
             let sender = ensure_signed(origin)?;
             //TODO：卧槽！新版是这样自定义参数的！！！！！！！！
             let  new_balance = <BalanceOf<T> as As<u64>>::sa(5000);
             // 新建账户并给他转入5000
             T::Currency::deposit_creating(&who,new_balance);
             Ok(())
        }

        /// 点击领取
        pub fn draw_reward_all(origin, _message: Vec<u8> , _signature: Vec<u8>) -> Result {
             let sender = ensure_signed(origin)?;

             ensure!(!Self::despositing_account().iter().find(|&t| t == &sender).is_none(), "Cannot draw if not depositing.");

             let reward = Self::count_draw_reward(sender.clone());
             let new_balance = <BalanceOf<T> as As<u64>>::sa(T::Balance::as_(reward));
             T::Currency::deposit_into_existing(&sender, new_balance).ok();
             Ok(())
        }

        /// withdraw
        /// origin, message: Vec<u8>, signature: Vec<u8>
         pub fn withdraw(origin, message: Vec<u8>, signature: Vec<u8>) -> Result {
            let sender = ensure_signed(origin)?;

            let validators = <session::Module<T>>::validators();
            ensure!(validators.contains(&sender),"Not validator");
            // 解析message --> hash  tag  id  amount
            let (_tx_hash,who,_amount,signature_hash) = Self::split_message(message.clone(),signature);
            let message_hash = Decode::decode(&mut &message.encode()[..]).unwrap();

            //check the validity and number of signatures
            runtime_io::print("开始检查签名");
            match  Self::check_signature(sender.clone(), message_hash, signature_hash, message_hash){
                Ok(y) =>  runtime_io::print("ok") ,
                Err(x) => return Err(x),
            }
            // ensure no repeat
            ensure!(!Self::despositing_account().iter().find(|&t| t == &who).is_none(), "Cannot deposit if not depositing.");
            ensure!(Self::intentions_withdraw().iter().find(|&t| t == &who).is_none(), "Cannot withdraw2 if already in withdraw2 queue.");
            runtime_io::print("============withdraw2===========");
            <IntentionsWithdraw<T>>::put({
                let mut v =  Self::intentions_withdraw();
                v.push(who.clone());
                v
            });

            // 发送一个event
            Self::deposit_event(RawEvent::AddWithdrawQueue(who));
            Ok(())
        }

        pub fn withdraw2(origin, hash: T::Hash, _tag: T::Hash, id: T::AccountId,_amount: T::Balance, signature: Vec<u8>) -> Result {
            //TODO:
            let sender = ensure_signed(origin)?;
            let who =  id;

            // ensure no repeat
            ensure!(!Self::despositing_account().iter().find(|&t| t == &who).is_none(), "Cannot deposit if not depositing.");
            ensure!(Self::intentions_withdraw().iter().find(|&t| t == &who).is_none(), "Cannot withdraw2 if already in withdraw2 queue.");

            let signature_hash =  Decode::decode(&mut &signature.encode()[..]).unwrap();
            runtime_io::print("开始检查withdraw签名");
            match  Self::check_signature(sender.clone(), hash, signature_hash, hash){
                Ok(y) =>  runtime_io::print("ok") ,
                Err(x) => return Err(x),
            }

            runtime_io::print("============withdraw2===========");
            <IntentionsWithdraw<T>>::put({
                let mut v =  Self::intentions_withdraw();
                v.push(who.clone());
                v
            });

            // 发送一个event
            Self::deposit_event(RawEvent::AddWithdrawQueue(who));
            Ok(())
        }

        /// set reward factor
        ///   0-5000 5000-50000 50000-500000 500000-->
        ///      x1         x2         x3      x3
        ///     y1      y2         y3          y4
        fn set_session_reward_factor(_origin,session: Vec<u32>, session_factor: Vec<u8>)-> Result{
            // session
            ensure!(session.len() >= 3,"not enough session arguments.at least 3");
            ensure!(session_factor.len() >= 4,"not enough session_factor arguments.at least 4");

            <RewardSessionValue<T>>::put(session);
            <RewardSessionFactor<T>>::put(session_factor);
            Ok(())
            // money
        }

        /// set reward factor
        ///   0-5000 5000-50000 50000-500000 500000-->
        ///      x1         x2         x3      x3
        ///     y1      y2         y3          y4
        fn set_balance_reward_factor(_origin, money: Vec<T::Balance>, money_factor: Vec<u8>)-> Result{

            ensure!(money.len() >= 3,"not enough money arguments.at least 3");
            ensure!(money_factor.len() >= 4,"not enough money_factor arguments.at least 4");

            <RewardBalanceValue<T>>::put(money);
            <RewardBalanceFactor<T>>::put(money_factor);

            Ok(())
            // money
        }
        /// set session lenth
        fn set_session_lenth(session_len: u64 ){
           ensure!(session_len >= 10,"the session lenth must larger than 10");
           <SessionLength<T>>::put(T::BlockNumber::sa(session_len));
        }

        pub fn draw_reward(origin,id: T::AccountId) -> Result{
             ensure!(!Self::despositing_account().iter().find(|&t| t == &id).is_none(), "Cannot draw if not depositing.");

             let new_balance = <BalanceOf<T> as As<u64>>::sa(T::Balance::as_(Self::count_draw_reward(id.clone())));
             match T::Currency::deposit_into_existing(&id, new_balance){
                 Err(x) => Err(x),
                 _ => Ok(()),
             }

        }
        /// a new session starts
        fn on_finalize(n: T::BlockNumber) {
            Self::check_rotate_session(n);
        }
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as Bank {
        /// bank & session
        /// record deposit info       AccountId -> message & signature
        DepositInfo get(deposit_info) : map  T::AccountId => (Vec<u8>,Vec<u8>);
        /// record depositing info of balance & session_time
        DespoitingAccount get(despositing_account): Vec<T::AccountId>;
        DespositingBalance get(despositing_banance): map T::AccountId => T::Balance;
        DespositingTime get(despositing_time): map T::AccountId => u32;

        /// All the accounts with a desire to deposit
        IntentionsDespositVec  get(intentions_desposit_vec) :  Vec<T::AccountId>;
        IntentionsDesposit  get(intentions_desposit): map T::AccountId => T::Balance;
           /// The block at which the `who`'s funds become entirely liquid.
        pub DepositBondage get(deposit_bondage): map T::AccountId => T::BlockNumber;
        /// All the accounts with a desire to withdraw
        IntentionsWithdraw  get(intentions_withdraw): Vec<T::AccountId>;

        /// Bank session reward factor
        RewardSessionValue  get(reward_session_value) config(): Vec<u32>;
        RewardSessionFactor  get(reward_session_factor) config(): Vec<u8>;
        /// Bank balance reward factor
        RewardBalanceValue  get(reward_balance_value) config(): Vec<T::Balance>;
        RewardBalanceFactor  get(reward_balance_factor) config(): Vec<u8>;

        //RewardFactorS get(reward_factor) config():map u64 => RewardFactor<u64>;


        ///Session module
        /// Block at which the session length last changed.
        LastLengthChange: Option<T::BlockNumber>;
        /// Current length of the session.
        pub SessionLength get(length) config(session_length): T::BlockNumber = T::BlockNumber::sa(10);

        /// The next session length.
        NextSessionLength: Option<T::BlockNumber>;
        /// Timestamp when current session started.
        pub CurrentStart get(current_start) build(|_| T::Moment::zero()): T::Moment;
        /// Current index of the session.
        pub CurrentIndex get(current_index) build(|_| T::BlockNumber::sa(0)): T::BlockNumber;

        /// 新功能 => 模拟chainX 把奖励记录下来，点击领取才发钱 的存储
        RewardRecord get(reward_record):  map T::AccountId => T::Balance;
        /// true -- 领取模式  false -- 自动发放模式
        EnableRewardRecord get(enable_record) config(): bool;
        /// 全链总余额
        TotalDespositingBalacne  get(total_despositing_balance) config(): T::Balance;
    }
}

decl_event! {
    pub enum Event<T> where
        <T as balances::Trait>::Balance,
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash,
        <T as system::Trait>::BlockNumber
    {
        Has(Hash),
        ///bank moduel
        /// All validators have been rewarded by the given balance.
        Reward(Balance),
        /// accountid added to the intentions to deposit queue
        AddDepositingQueue(AccountId),
        /// intentions to withdraw
        AddWithdrawQueue(AccountId),
        /// a new seesion start
        NewRewardSession(BlockNumber),

    }
}

impl<T: Trait> Module<T> {
    fn split_message(
        message: Vec<u8>,
        signature: Vec<u8>,
    ) -> (T::Hash, T::AccountId, u64, T::Hash) {
        // 解析message --> hash  tag  id  amount
        let mut messagedrain = message.clone();

        let hash: Vec<_> = messagedrain.drain(0..32).collect();

        let tx_hash = Decode::decode(&mut &hash.encode()[..]).unwrap();

        let signature_hash = Decode::decode(&mut &signature.encode()[..]).unwrap();

        let id: Vec<_> = messagedrain.drain(32..64).collect();
        let who: T::AccountId = Decode::decode(&mut &id.encode()[..]).unwrap();
        let amount_vec: Vec<u8> = messagedrain.drain(32..40).collect();
        let mut amount: u64 = 0;
        let mut i = 0;
        amount_vec.iter().for_each(|x| {
            let exp = (*x as u64) ^ i;
            amount = amount + exp;
            i = i + 1;
        });

        //ensure the signature is valid
        let mut tx_hash_to_check: [u8; 65] = [0; 65];
        tx_hash_to_check.clone_from_slice(&hash);
        let mut signature_hash_to_check: [u8; 32] = [0; 32];
        signature_hash_to_check.clone_from_slice(&signature);
        Self::check_secp512(&tx_hash_to_check, &signature_hash_to_check).is_ok();

        return (tx_hash, who, amount, signature_hash);
    }

    /// Hook to be called after transaction processing.  间隔一段时间才触发 rotate_session
    pub fn check_rotate_session(block_number: T::BlockNumber) {
        // do this last, after the staking system has had chance to switch out the authorities for the
        // new set.
        // check block number and call next_session if necessary.
        let is_final_block =
            ((block_number - Self::last_length_change()) % Self::length()).is_zero();
        /*
        let (should_end_session, apply_rewards) = <ForcingNewSession<T>>::take()
            .map_or((is_final_block, is_final_block), |apply_rewards| (true, apply_rewards));
        */
        if true {
            runtime_io::print("--------安排上了04-01--------");
            Self::rotate_session(is_final_block, true);
        }
    }

    /// The last length change, if there was one, zero if not.  查看lenth间隔长度
    pub fn last_length_change() -> T::BlockNumber {
        <LastLengthChange<T>>::get().unwrap_or_else(T::BlockNumber::zero)
    }

    /// Move onto next session: register the new authority set.
    /// 把新的 depositingqueue 加入 实际depositing列表   或者 把不存钱的账户从列表里删除
    /// 并且根据其存的金额 之后每个session都对列表里的人存的钱发一定比例到他的balance里
    pub fn rotate_session(is_final_block: bool, _apply_rewards: bool) {
        runtime_io::print("开始调整depositing列表，同时开始给他们发钱");
        let now = <timestamp::Module<T>>::get();
        let _time_elapsed = now.clone() - Self::current_start();
        let session_index = <CurrentIndex<T>>::get() + One::one();
        Self::deposit_event(RawEvent::NewRewardSession(session_index));

        // Increment current session index.
        <CurrentIndex<T>>::put(session_index);
        <CurrentStart<T>>::put(now);
        // Enact session length change.
        let len_changed = if let Some(next_len) = <NextSessionLength<T>>::take() {
            <SessionLength<T>>::put(next_len);
            true
        } else {
            false
        };
        if len_changed || !is_final_block {
            let block_number = <system::Module<T>>::block_number();
            <LastLengthChange<T>>::put(block_number);
        }
        Self::adjust_deposit_list();

        // 1直接发奖励至账户  2点击领取奖励
        match Self::enable_record() {
            true => Self::reward_deposit_record(),
            _ => Self::reward_deposit(),
        }
    }

    fn adjust_deposit_list() {
        //修改全部的表 deposit 部分   已经判断过重复了所以直接天加
        let mut int_des_vec = Self::intentions_desposit_vec();
        let mut des_vec = Self::despositing_account();
        while let Some(who) = int_des_vec.pop() {
            runtime_io::print("========add===========");
            //更新正在抵押人列表
            let balances = <IntentionsDesposit<T>>::get(who.clone());
            <DespositingBalance<T>>::insert(who.clone(), balances);
            <DespositingTime<T>>::insert(who.clone(), 0);
            des_vec.push(who.clone());

            let total_deposit_balance = <TotalDespositingBalacne<T>>::get();
            <TotalDespositingBalacne<T>>::put(balances + total_deposit_balance);
            <IntentionsDesposit<T>>::remove(who);
        }
        <DespoitingAccount<T>>::put(des_vec);
        <IntentionsDespositVec<T>>::put(int_des_vec);

        /////////////////////////////////////////////////////////a
        let mut des_vec2 = Self::despositing_account();
        let mut vec_with = Self::intentions_withdraw();
        while let Some(who) = vec_with.pop() {
            runtime_io::print("========remove===========");
            //增加 despoit 记录  同时创建session记录
            let balances = <DespositingBalance<T>>::get(who.clone());

            <DespositingBalance<T>>::remove(who.clone());
            des_vec2.push(who.clone());
            //抵押总余额
            let total_deposit_balance = <TotalDespositingBalacne<T>>::get();
            <TotalDespositingBalacne<T>>::put(balances + total_deposit_balance);

            <DespositingTime<T>>::remove(who.clone());
        }
        <IntentionsWithdraw<T>>::put(vec_with);
        <DespoitingAccount<T>>::put(des_vec2);

        runtime_io::print("对表里的session time进行更新");
        //对表进行session time更新
        Self::despositing_account()
            .iter()
            .enumerate()
            .for_each(|(_i, v)| {
                <DespositingTime<T>>::insert(v, Self::despositing_time(v) + 1);
            });
    }

    /// 新功能 => 模拟chainX 把奖励记录下来，点击领取才发钱
    fn count_draw_reward(accountid: T::AccountId) -> T::Balance {
        let mut reward = <RewardRecord<T>>::get(accountid.clone());

        <RewardRecord<T>>::insert(accountid, T::Balance::sa(0));

        reward
    }
    /// 新功能 => 模拟chainX 把奖励记录下来，点击领取才发钱
    fn reward_deposit_record() {
        //循环历遍一下所有deposit列表，分别对每个账户的奖励进行记录
        //首先判断session 决定一个 时间比率
        //再判断balance 决定一个 存款比率
        //两个比率结合起来决定一个 乘积因子Xbalance => 然后往账户的记录上记录奖励额度
        runtime_io::print("发钱====发到记录里面！！！！！！！！！！！！");
        Self::despositing_account()
            .iter()
            .enumerate()
            .for_each(|(_i, v)| {
                let reward = Self::reward_set(
                    v.clone(),
                    <DespositingTime<T>>::get(v),
                    <DespositingBalance<T>>::get(v),
                );
                let now_reward = <RewardRecord<T>>::get(v);
                <RewardRecord<T>>::insert(v, reward + now_reward);
            });
    }

    ///每个周期直接给指定的抵押账户发奖励的钱
    fn reward_deposit() {
        //循环历遍一下所有deposit列表，分别对每个账户进行单独的发钱处理
        //首先判断session 决定一个 时间比率
        //再判断balance 决定一个 存款比率
        //两个比率结合起来决定一个 乘积因子Xbalance => 然后往账户发
        runtime_io::print("发钱发钱发钱发钱发钱发钱发钱发钱发钱发钱");
        Self::despositing_account()
            .iter()
            .enumerate()
            .for_each(|(_i, v)| {
                //TODO:测试时候注释
                runtime_io::print("================TEST==================");

                let reward = Self::reward_set(
                    v.clone(),
                    <DespositingTime<T>>::get(v),
                    <DespositingBalance<T>>::get(v),
                );
                //TODO: reward
                // let _ = <balances::Module<T>>::reward(v, reward);
            });
    }

    // RewardSessionValue  get(reward_session_value): Vec<u64>
    // RewardSessionFactor  get(reward_session_factor): Vec<u64>
    fn reward_set(who: T::AccountId, session: u32, money: T::Balance) -> T::Balance {
        let session_value = Self::reward_session_value();
        let session_factor = Self::reward_session_factor();

        let x1 = session_value[0];
        let x2 = session_value[1];
        let x3 = session_value[2];
        let y1 = session_factor[0];
        let y2 = session_factor[1];
        let y3 = session_factor[2];
        let y4 = session_factor[3];

        #[warn(unused_assignments)]
        let mut final_se = 0;
        if session <= x1 {
            final_se = y1;
        } else if session <= x2 {
            final_se = y2;
        } else if session <= x3 {
            final_se = y3;
        } else {
            final_se = y4;
        }

        let balance_value = Self::reward_balance_value();
        let balance_factor = Self::reward_balance_factor();

        let xx1 = balance_value[0];
        let xx2 = balance_value[1];
        let xx3 = balance_value[2];
        let yy1 = balance_factor[0];
        let yy2 = balance_factor[1];
        let yy3 = balance_factor[2];
        let yy4 = balance_factor[3];

        let mut final_ba = 0;
        if money <= xx1 {
            final_ba = yy1;
        } else if money <= xx2 {
            final_ba = yy2;
        } else if money <= xx3 {
            final_ba = yy3;
        } else {
            final_ba = yy4;
        }

        <DespositingBalance<T>>::get(who) * T::Balance::sa((final_se * final_ba) as u64)
    }

    fn check_signature(
        who: T::AccountId,
        tx: T::Hash,
        signature: T::Hash,
        message_hash: T::Hash,
    ) -> Result {
        //ensure enough signature
        <signcheck::Module<T>>::check_signature(who, tx, signature, message_hash)
    }

    pub fn check_secp512(tx: &[u8; 65], signature: &[u8; 32]) -> Result {
        runtime_io::print("asd");
        //TODO: if runtime_io::secp256k1_ecdsa_recover(signature,tx).is_ok(){ } else {
        //  return Err(()); }
        //TODO:
        Ok(())
    }

    pub fn balancetest(x1: T::Balance, x2: T::Balance) {
        let aa = T::Balance::sa(4);
        <T as balances::Trait>::Balance::sa(5);
        x1.checked_add(&x2);
    }

    pub fn banalncenewt(amount1: BalanceOf<T>, amount2: BalanceOf<T>, amount3: T::Balance) {
        amount1.checked_add(&amount2);
        let xx = T::Balance::as_(amount3);
        //let aaa = <BalanceOf<T>>::sa(56) * amount3;

        //let a: BalanceOf<T> = xx;
        //let amount3 = amount1 + T::Currency::Balance( T::Balance::sa(5000));
    }
}
