#[macro_use]
extern crate error_chain;
extern crate toml;
extern crate tokio_timer;
#[macro_use]
extern crate futures;
extern crate web3;
extern crate tokio;
extern crate tokio_core;
#[macro_use]
extern crate log;

#[cfg_attr(test, macro_use)]
extern crate serde_json;

#[macro_use]
extern crate serde_derive;
extern crate contracts;
extern crate signer;
extern crate ethabi;
extern crate rustc_hex;

extern crate node_runtime;
extern crate sr_primitives as runtime_primitives;
extern crate substrate_client as client;
extern crate substrate_network as network;
extern crate substrate_keystore as keystore;
extern crate substrate_primitives as primitives;
extern crate substrate_transaction_pool as transaction_pool;

extern crate rustc_serialize;
use rustc_serialize::json;

extern crate curl;
use curl::easy::{Easy2, Handler, WriteError};

#[cfg(test)]
#[macro_use]
mod test;
#[cfg(test)]
extern crate jsonrpc_core;
#[cfg(test)]
pub use test::{MockTransport, MockClient};


#[macro_use]
mod macros;
pub mod error;
//mod fixed_number;
pub mod log_stream;
pub mod block_number_stream;
pub mod vendor;
pub mod events;
pub mod message;
mod state;
mod utils;

use std::str::FromStr;
use message::{RelayMessage,RelayType};
use tokio_core::reactor::Core;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender};
use std::path::{Path, PathBuf};
use error::{ResultExt};
use vendor::Vendor;
use signer::{SecretKey, RawTransaction, KeyPair, PrivKey};
use state::{StateStorage};
use network::SyncProvider;
use futures::{Future, Stream};
use keystore::Store as Keystore;
use runtime_primitives::codec::{Decode, Encode, Compact};
use runtime_primitives::generic::{BlockId, Era};
use runtime_primitives::traits::{Block, BlockNumberToHash, ProvideRuntimeApi};
use client::{runtime_api::Core as CoreApi, BlockchainEvents, blockchain::HeaderBackend};
use primitives::storage::{StorageKey, StorageData, StorageChangeSet};
use primitives::{ed25519::Pair as TraitPait, ed25519::Pair};
use transaction_pool::txpool::{self, Pool as TransactionPool, ExtrinsicFor};
use node_runtime::{
    Call, UncheckedExtrinsic, EventRecord, Event, MatrixCall, BankCall, matrix::*, ExrCall,
    VendorApi,exchangerate,ExchangeRateApi
};
use node_runtime::{Balance, Hash, AccountId, Nonce as Index, BlockNumber};
use web3::{
    api::Namespace, 
    types::{Address, Bytes, H256, U256},
};
use std::marker::{Send, Sync};
use log_stream::ChainAlias;

const MAX_PARALLEL_REQUESTS: usize = 10;

pub trait SuperviseClient{
    fn submit(&self, message: RelayMessage);
}

pub struct PacketNonce<B> where B: Block{
    pub nonce: Index, // to control nonce.
    pub last_block: BlockId<B>,
}

pub struct Supervisor<A, B, C, N> where
    A: txpool::ChainApi,
    B: Block
{
    pub client: Arc<C>,
    pub pool: Arc<TransactionPool<A>>,
    pub network: Arc<N>,
    pub key: Pair,
    pub eth_key: SecretKey,
    pub phantom: std::marker::PhantomData<B>,
    // pub queue: Vec<(RelayMessage, u8)>,
    pub packet_nonce: Arc<Mutex<PacketNonce<B>>>,
}

impl<A, B, C, N> Supervisor<A, B, C, N> where
    A: txpool::ChainApi<Block = B>,
    B: Block,
    C: BlockchainEvents<B> + HeaderBackend<B> + BlockNumberToHash + ProvideRuntimeApi,
    N: SyncProvider<B>,
    C::Api: VendorApi<B>
{
    /// get nonce with atomic
    fn get_nonce(&self) -> Index {
        let mut p_nonce = self.packet_nonce.lock().unwrap();
        let info = self.client.info().unwrap();
        let at = BlockId::Hash(info.best_hash);
        if p_nonce.last_block == at {
            p_nonce.nonce = p_nonce.nonce + 1;
        } else {
            p_nonce.nonce = self.client.runtime_api().account_nonce(&at, self.key.public().0.into()).unwrap();
            p_nonce.last_block = at;
        }

        p_nonce.nonce
    }
}

impl<A, B, C, N> SuperviseClient for Supervisor<A, B, C, N> where
    A: txpool::ChainApi<Block = B>,
    B: Block,
    C: BlockchainEvents<B> + HeaderBackend<B> + BlockNumberToHash + ProvideRuntimeApi,
    N: SyncProvider<B>,
    C::Api: VendorApi<B> + CoreApi<B>
{
    fn submit(&self, message: RelayMessage) {
        let local_id: AccountId = self.key.public().0.into();
        let info = self.client.info().unwrap();
        let at = BlockId::Hash(info.best_hash);
        // let auths = self.client.runtime_api().authorities(&at).unwrap();
        // if auths.contains(&AuthorityId::from(self.key.public().0)) {
        if self.client.runtime_api().is_authority(&at, &self.key.public().0.into()).unwrap() {
            let nonce = self.get_nonce();
            let signature = signer::sign_message(&self.eth_key, &message.raw).into();

            let function =  match message.ty {
                    RelayType::Ingress => Call::Matrix(MatrixCall::ingress(message.raw, signature)),
                    RelayType::Egress => Call::Matrix(MatrixCall::egress(message.raw, signature)),
                    RelayType::Deposit => Call::Bank(BankCall::deposit(message.raw, signature)),
                    RelayType::Withdraw => Call::Bank(BankCall::withdraw(message.raw, signature)),
                    RelayType::SetAuthorities => Call::Matrix(MatrixCall::reset_authorities(message.raw, signature)),
                    RelayType::ExchangeRate => Call::Exchangerate(ExrCall::check_exchange(message.raw,signature)),
            };

            let payload = (
                Compact::<Index>::from(nonce),  // index/nonce
                function, //function
                Era::immortal(),  
                self.client.genesis_hash(),
            );
            
            let signature = self.key.sign(&payload.encode());
            let extrinsic = UncheckedExtrinsic::new_signed(
                payload.0.into(),
                payload.1,
                local_id.into(),
                signature.into(),
                payload.2
            );

            let xt: ExtrinsicFor<A> = Decode::decode(&mut &extrinsic.encode()[..]).unwrap();
            println!("extrinsic {:?}", xt);
            println!("@submit transaction {:?}",self.pool.submit_one(&at, xt));
        }
    }
}

#[derive(Clone)]
pub struct RunStrategy {
    pub listener: bool,
    pub sender: bool,
}

#[derive(Clone)]
pub struct VendorServiceConfig {
    pub kovan_url: String,
    pub ropsten_url: String,
    pub kovan_address: String,
    pub ropsten_address: String,
    pub db_path: String,
    pub eth_key: String,
    pub strategy: RunStrategy,
}

pub struct SideListener<V> {
    pub url: String,
    pub contract_address: Address,
    pub db_file: PathBuf,
    pub spv: Arc<V>,
    pub enable: bool,
    pub chain: ChainAlias,
}

fn print_err(err: error::Error) {
    let message = err
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n\nCaused by:\n  ");
    error!("{}", message);
}

fn is_err_time_out(err: &error::Error) -> bool {
    err.iter().any(|e| e.to_string() == "Request timed out")
}

impl<V> SideListener<V> where 
    V: SuperviseClient + Send + Sync + 'static
{
    fn start(self) {
        // return directly.
        if !self.enable {return;}
        // TODO hook the event of http disconnect to keep run.
        std::thread::spawn(move ||{
            let mut event_loop = Core::new().unwrap();
            loop {
                let transport = web3::transports::Http::with_event_loop(
                        &self.url,
                        &event_loop.handle(),
                        MAX_PARALLEL_REQUESTS,
                    )
                    .chain_err(|| {format!("Cannot connect to ethereum node at {}", self.url)}).unwrap();

                if !self.db_file.exists() {
                    std::fs::File::create(&self.db_file).expect("failed to create the storage file of state.");
                }
                let mut storage = StateStorage::load(self.db_file.as_path()).unwrap();
                let vendor = Vendor::new(&transport, self.spv.clone(), storage.state.clone(), self.contract_address, self.chain)
                                    .and_then(|state| {
                                        storage.save(&state)?;
                                        Ok(())
                                    })
                                    .for_each(|_| Ok(()));
                match event_loop.run(vendor) {
                    Ok(s) => {
                        info!("{:?}", s);
                        break;
                    }
                    Err(err) => {
                        if is_err_time_out(&err) {
                            error!("\nreqeust time out sleep 5s and try again.\n");
                            std::thread::sleep_ms(5000);
                        } else {
                            print_err(err);
                        }
                    }
                }
            }
        });
    }
}

///////////////////////////////////////////////////////////////////////
#[derive(RustcDecodable, RustcEncodable)]
pub struct exchange_rate{
    ticker : String,
    exchangeName : String,
    base : String,
    currency : String,
    symbol : String,
    high : f64,
    open : f64,
    close : f64,
    low : f64,
    vol : f64,
    degree : f64,
    value : f64,
    changeValue : f64,
    commissionRatio : f64,
    quantityRatio : f64,
    turnoverRate : f64,
    dateTime : u64,
}

fn parse_exchange_rate(content : String) -> (f64,u64) {
    // Deserialize using `json::decode`
    // 将json字符串中的数据转化成Struct对应的数据，相当于初始化
    let decoded: exchange_rate = json::decode(&content).unwrap();
    println!("exchange_rate {:?}",decoded.close);
    println!("exchange_rate time {:?}",decoded.dateTime);
    (decoded.close,decoded.dateTime)
}
 // let local_id: AccountId = self.key.public().0.into();
 pub trait ExchangeTrait{
     fn check_validators(&self) -> Result<u32,u32>;
 }

impl <A,B,Q> ExchangeTrait for Exchange<A,B,Q> where
    A: txpool::ChainApi<Block = B>+ 'static,
    B: Block,
    Q: BlockchainEvents<B> + HeaderBackend<B> + BlockNumberToHash + ProvideRuntimeApi + 'static,
    Q::Api: VendorApi<B>,
{
    fn check_validators(&self) -> Result<u32,u32> {
        let info = self.client.info().unwrap();
        let at = BlockId::Hash(info.best_hash);
        let accountid = self.accountid;
        self.client.runtime_api().check_validator(&at,accountid).unwrap();
        Ok(1)
    }
}

pub struct Exchange <A,B,C> where
    A: txpool::ChainApi,
    B: Block

{
    pub client: Arc<C>,
    pub pool: Arc<TransactionPool<A>>,
    pub accountid : AccountId,
    //pub phantom: std::marker::PhantomData<B>,
    pub packet_nonce: Arc<Mutex<PacketNonce<B>>>,
}

use std::thread; // 线程
use std::time::Duration; // 时间差

/// oracle 获取ETHUSD等等的汇率
impl <A,B,Q> Exchange <A,B,Q> where
    A: txpool::ChainApi<Block = B>+ 'static,
    B: Block,
    Q: BlockchainEvents<B> + HeaderBackend<B> + BlockNumberToHash + ProvideRuntimeApi + 'static,
    Q::Api: VendorApi<B>,
{
    fn start(self)  {
       // let (sender, receiver) = channel();
        std::thread::spawn(move || {
            println!("1111111111111111111111");
            struct Collector(Vec<u8>);
            impl Handler for Collector {
                fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
                    self.0.extend_from_slice(data);
                    Ok(data.len())
                }
            }
            loop {
                let mut easy = Easy2::new(Collector(Vec::new()));
                easy.get(true).unwrap();
                easy.url("http://api.coindog.com/api/v1/tick/BITFINEX:ETHUSD?unit=cny").unwrap();

                //let event = receiver.recv().unwrap();
                // Judging whether the validators is legitimate
                if  self.check_validators().is_ok(){
                    // perform the http get method and fetch the responsed
                    easy.perform().unwrap();

                    if  easy.response_code().unwrap() == 200 {
                        let contents = easy.get_ref();
                        let contents_string = String::from_utf8_lossy(&contents.0).to_string() ;
                        println!("exchange_rate <---> {}",contents_string);
                        let (exchange_rate,time) = parse_exchange_rate(contents_string);
                        //TODO:accountid
                        //use node_runtime::exchangerate::Trait as T;
                        //TODO 保存数据<exchangerate::Module<T>>::check_signature( self.accountid ,(exchange_rate*10000.0f64) as u64 ,time);
                    }else{
                        println!("获取汇率失败");
                    }
                };
                // query the exchange rate every 5 seconds
                thread::sleep(Duration::from_secs(5));

            }
        });
       // sender
    }
}

///////////////////////////////////////////////
type SignData = Vec<u8>;

struct SignContext {
    nonce: U256,
    contract_address: Address,
    payload: Vec<u8>,
}

trait SenderProxy{
    fn sign(&self, context: SignContext) -> Vec<u8>;
    // fn send(event_loop: &mut Core, transport: &Transport) -> Result<()>;
}

struct EthProxy {
    pair: KeyPair,
}

impl SenderProxy for EthProxy {
    fn sign(&self, context: SignContext) -> Vec<u8> {
        let transaction = RawTransaction {
                                    nonce: context.nonce.into(),
                                    to: Some(context.contract_address),
                                    value: 0.into(),
                                    data: context.payload,
                                    gas_price: 2000000000.into(),
                                    gas: 41000.into(),
                                };
        let sec: &SecretKey = unsafe { std::mem::transmute(self.pair.privkey()) };
        let data = signer::sign_transaction(&sec, &transaction);
        data
    }
}

struct AbosProxy {
    pair: KeyPair,
}

impl SenderProxy for AbosProxy {
    fn sign(&self, context: SignContext) -> Vec<u8> {
        
    }
}

struct SideSender<P>{
    name: String,
    url: String,
    contract_address: Address,
    pair: KeyPair,
    enable: bool,
    proxy: Arc<P>,
}

impl<P> SideSender<P> where P: SenderProxy + Send + Sync + 'static {
    fn start(self) -> Sender<RawEvent<Balance, AccountId, Hash, BlockNumber>> {
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let mut event_loop = Core::new().unwrap();
            let transport = web3::transports::Http::with_event_loop(
                    &self.url,
                    &event_loop.handle(),
                    MAX_PARALLEL_REQUESTS,
                )
                .chain_err(|| {format!("Cannot connect to ethereum node at {}", self.url)}).unwrap();

            // get nonce
            let authority_address: Address = self.pair.address();
            let nonce_future  = web3::api::Eth::new(&transport).transaction_count(authority_address, None);
            let mut nonce = event_loop.run(nonce_future).unwrap();
            info!("eth nonce: {}", nonce);
            loop {
                let event = receiver.recv().unwrap();

                if !self.enable {continue;}

                let data = match event {
                    RawEvent::Ingress(message, signatures) => {
                        info!("ingress message: {:?}, signatures: {:?}", message, signatures);
                        let payload = contracts::bridge::functions::release::encode_input(message, signatures);
                        Some(payload)
                    },
                    _ => {
                        None
                    }
                };
                if let Some(payload) = data {
                    let context = SignContext {
                        nonce: nonce,
                        contract_address: self.contract_address,
                        payload: payload,
                    };
                    let data = self.proxy.sign(context);
                    let future = web3::api::Eth::new(&transport).send_raw_transaction(Bytes::from(data));
                    let hash = event_loop.run(future).unwrap();
                    info!("send to eth transaction hash: {:?}", hash);
                    nonce += 1.into();
                }
            }
        });

        sender
    }
}

/// Start the supply worker. The returned future should be run in a tokio runtime.
pub fn start_vendor<A, B, C, N>(
    config: VendorServiceConfig,
    network: Arc<N>,
    client: Arc<C>,
    pool: Arc<TransactionPool<A>>,
    keystore: &Keystore,
    on_exit: impl Future<Item=(),Error=()>,
) -> impl Future<Item=(),Error=()> where
    A: txpool::ChainApi<Block = B> + 'static,
    B: Block + 'static,
    C: BlockchainEvents<B> + HeaderBackend<B> + BlockNumberToHash + ProvideRuntimeApi + 'static,
    N: SyncProvider<B> + 'static,
    C::Api: VendorApi<B>,
{
    let key = keystore.load(&keystore.contents().unwrap()[0], "").unwrap();
    let key2 = keystore.load(&keystore.contents().unwrap()[0], "").unwrap();
    let kovan_address = Address::from_str(&config.kovan_address).unwrap();
    let ropsten_address = Address::from_str(&config.ropsten_address).unwrap();
    let eth_key = SecretKey::from_str(&config.eth_key).unwrap();
    let eth_pair = KeyPair::from_privkey(PrivKey::from_slice(&eth_key[..]));
    info!("ss58 account: {:?}, eth account: {}", key.public().to_ss58check(), eth_pair);

    let info = client.info().unwrap();
    let at = BlockId::Hash(info.best_hash);
    let packet_nonce = PacketNonce {
            nonce: client.runtime_api().account_nonce(&at, key.public().0.into()).unwrap(),
            last_block: at,
        };
    let packet_nonce2 = PacketNonce {
        nonce: client.runtime_api().account_nonce(&at, key.public().0.into()).unwrap(),
        last_block: at,
    };

    let spv = Arc::new(Supervisor {
        client: client.clone(),
        pool: pool.clone(),
        network: network.clone(),
        key: key,
        eth_key: eth_key.clone(),
        packet_nonce: Arc::new(Mutex::new(packet_nonce)),
        phantom: std::marker::PhantomData,
    });
    
    //new a thread to listen kovan network
    SideListener {
        url: config.kovan_url.clone(), 
        db_file: Path::new(&config.db_path).join("kovan_storage.json"),
        contract_address: kovan_address,
        spv: spv.clone(),
        enable: config.strategy.listener,
        chain: ChainAlias::ETH,
    }.start();

    //new a thread to listen ropsten network
    SideListener {
        url: config.ropsten_url.clone(),
        db_file:  Path::new(&config.db_path).join("ropsten_storage.json"),
        contract_address: ropsten_address,
        spv: spv.clone(),
        enable: config.strategy.listener,
        chain: ChainAlias::ETH,
    }.start();

    // A thread that send transaction to ETH
    let kovan_sender = SideSender {
        name: "ETH_Kovan".to_string(),
        url: config.kovan_url.clone(),
        contract_address: kovan_address,
        pair: eth_pair.clone(),
        enable: config.strategy.sender,
        proxy: Arc::new(EthProxy{ pair: eth_pair.clone() }),
    }.start();
    
    let ropsten_sender = SideSender {
        name: "ETH_Ropsten".to_string(),
        url: config.ropsten_url.clone(),
        contract_address: ropsten_address,
        pair: eth_pair.clone(),
        enable: config.strategy.sender,
        proxy: Arc::new(EthProxy{ pair: eth_pair.clone() }),
    }.start();

    // exchange
    let ext = Exchange{
        client: client.clone(),
        pool: pool.clone(),
        accountid: key2.public().0.into(),
        packet_nonce: Arc::new(Mutex::new(packet_nonce2)),
    };

    ext.start();


    let eth_kovan_tag = H256::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
    let eth_ropsten_tag = H256::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap();
    // how to fetch real key?
    let events_key = StorageKey(primitives::twox_128(b"System Events").to_vec());
    let storage_stream = client.storage_changes_notification_stream(Some(&[events_key])).unwrap()
    .map(|(block, changes)| StorageChangeSet { block, changes: changes.iter().cloned().collect()})
    .for_each(move |change_set| {
        let records: Vec<Vec<EventRecord<Event>>> = change_set.changes
            .iter()
            .filter_map(|(_, mbdata)| if let Some(StorageData(data)) = mbdata {
                Decode::decode(&mut &data[..])
            } else { None })
            .collect();
        let events: Vec<Event> = records
            .concat()
            .iter()
            .cloned()
            .map(|r| r.event)
            .collect();
        events.iter().for_each(|event| {
            if let Event::matrix(e) = event {
                match e {
                    RawEvent::Ingress(message, signatures) => {
                        println!("raw event ingress: {:?}, {:?}", message, signatures);
                        events::IngressEvent::from_bytes(message).map(|ie| {
                            if ie.tag == eth_kovan_tag {
                                kovan_sender.send(e.clone()).unwrap();
                            } else if ie.tag == eth_ropsten_tag {
                                ropsten_sender.send(e.clone()).unwrap();
                            } else {
                                warn!("unknown event tag of ingress: {:?}", ie.tag);
                            }
                        }).map_err(|err| {
                            warn!("unexpected format of ingress, message {:?}", message);
                        });
                    },
                    _ => {}
                };
            }
        });
        Ok(())
    });

    storage_stream
            .map(|_|())
            .select(on_exit)
            .then(|_| {Ok(())})
}
