// Copyright 2018-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

#![warn(unused_extern_crates)]

//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::sync::Arc;
use std::time::Duration;

use crate::params::VendorCmd;
use client::{self, LongestChain};
use consensus::{import_queue, start_aura, AuraImportQueue, NothingExtra, SlotDuration};
use grandpa;
use inherents::InherentDataProviders;
use log::info;
use network::construct_simple_protocol;
use node_executor;
use node_primitives::Block;
use node_runtime::{GenesisConfig, RuntimeApi};
use primitives::{ed25519, Pair as PairT};
use signer::Keyring;
use substrate_service::construct_service_factory;
use substrate_service::TelemetryOnConnect;
use substrate_service::{
    FactoryFullConfiguration, FullBackend, FullClient, FullComponents, FullExecutor, LightBackend,
    LightClient, LightComponents, LightExecutor, TaskExecutor,
};
use transaction_pool::{self, txpool::Pool as TransactionPool};
use vendor::{start_vendor, VendorServiceConfig};

construct_simple_protocol! {
    /// Demo protocol attachment for substrate.
    pub struct NodeProtocol where Block = Block { }
}

/// Node specific configuration
pub struct NodeConfig<F: substrate_service::ServiceFactory> {
    /// grandpa connection to import block
    // FIXME #1134 rather than putting this on the config, let's have an actual intermediate setup state
    pub grandpa_import_setup: Option<(
        Arc<grandpa::BlockImportForService<F>>,
        grandpa::LinkHalfForService<F>,
    )>,
    inherent_data_providers: InherentDataProviders,
    pub custom_args: VendorCmd,
}

impl<F> Default for NodeConfig<F>
where
    F: substrate_service::ServiceFactory,
{
    fn default() -> NodeConfig<F> {
        NodeConfig {
            grandpa_import_setup: None,
            inherent_data_providers: InherentDataProviders::new(),
            custom_args: VendorCmd::default(),
        }
    }
}

construct_service_factory! {
    struct Factory {
        Block = Block,
        RuntimeApi = RuntimeApi,
        NetworkProtocol = NodeProtocol { |config| Ok(NodeProtocol::new()) },
        RuntimeDispatch = node_executor::Executor,
        FullTransactionPoolApi = transaction_pool::ChainApi<client::Client<FullBackend<Self>, FullExecutor<Self>, Block, RuntimeApi>, Block>
            { |config, client| Ok(TransactionPool::new(config, transaction_pool::ChainApi::new(client))) },
        LightTransactionPoolApi = transaction_pool::ChainApi<client::Client<LightBackend<Self>, LightExecutor<Self>, Block, RuntimeApi>, Block>
            { |config, client| Ok(TransactionPool::new(config, transaction_pool::ChainApi::new(client))) },
        Genesis = GenesisConfig,
        Configuration = NodeConfig<Self>,
        FullService = FullComponents<Self> {
                    |config: FactoryFullConfiguration<Self>, executor: TaskExecutor| {
                        let db_path = config.database_path.clone();
                        let keyring = config.keys.first().map_or(Keyring::default(), |key| Keyring::from(key.as_bytes()));
                        let run_args = config.custom.custom_args.clone();
                        info!("eth signer key: {}", keyring.to_hex());
                        match FullComponents::<Factory>::new(config, executor.clone()) {
                            Ok(service) => {
                                executor.spawn(start_vendor(
                                    VendorServiceConfig { kovan_url: "https://kovan.infura.io/v3/5b83a690fa934df09253dd2843983d89".to_string(),
                                                        ropsten_url: "https://ropsten.infura.io/v3/5b83a690fa934df09253dd2843983d89".to_string(),
                                                        kovan_address: "690aB411ca08bB0631C49513e10b29691561bB08".to_string(),
                                                        ropsten_address: "631b6b933Bc56Ebd93e4402aA5583650Fcf74Cc7".to_string(),
                                                        db_path: db_path,
                                                        eth_key: keyring.to_hex(), // sign message
                                                        strategy: run_args.into(),
                                                        },
                                    service.network(),
                                    service.client(),
                                    service.transaction_pool(),
                                    service.keystore(),
                                    service.on_exit(),
                                ));
                                return Ok(service)
                            },
                            Err(err) => return Err(err),
                        }
                    }
                },
        AuthoritySetup = {
            |mut service: Self::FullService, executor: TaskExecutor, local_key: Option<Arc<ed25519::Pair>>| {
                let (block_import, link_half) = service.config.custom.grandpa_import_setup.take()
                    .expect("Link Half and Block Import are present for Full Services or setup failed before. qed");

                if let Some(ref key) = local_key {
                    info!("Using authority key {}", key.public());
                    let proposer = Arc::new(substrate_basic_authorship::ProposerFactory {
                        client: service.client(),
                        transaction_pool: service.transaction_pool(),
                        inherents_pool: service.inherents_pool(),
                    });

                    let client = service.client();
                    executor.spawn(start_aura(
                        SlotDuration::get_or_compute(&*client)?,
                        key.clone(),
                        client,
                        service.select_chain(),
                        block_import.clone(),
                        proposer,
                        service.network(),
                        service.on_exit(),
                        service.config.custom.inherent_data_providers.clone(),
                        service.config.force_authoring,
                    )?);

                    info!("Running Grandpa session as Authority {}", key.public());
                }

                let local_key = if service.config.disable_grandpa {
                    None
                } else {
                    local_key
                };

                let config = grandpa::Config {
                    local_key,
                    // FIXME #1578 make this available through chainspec
                    gossip_duration: Duration::from_millis(333),
                    justification_period: 4096,
                    name: Some(service.config.name.clone())
                };

                match config.local_key {
                    None => {
                        executor.spawn(grandpa::run_grandpa_observer(
                            config,
                            link_half,
                            service.network(),
                            service.on_exit(),
                        )?);
                    },
                    Some(_) => {
                        let telemetry_on_connect = TelemetryOnConnect {
                          on_exit: Box::new(service.on_exit()),
                          telemetry_connection_sinks: service.telemetry_on_connect_stream(),
                          executor: &executor,
                        };
                        let grandpa_config = grandpa::GrandpaParams {
                          config: config,
                          link: link_half,
                          network: service.network(),
                          inherent_data_providers: service.config.custom.inherent_data_providers.clone(),
                          on_exit: service.on_exit(),
                          telemetry_on_connect: Some(telemetry_on_connect),
                        };
                        executor.spawn(grandpa::run_grandpa_voter(grandpa_config)?);
                    },
                }

                Ok(service)
            }
        },
        LightService = LightComponents<Self>
            { |config, executor| <LightComponents<Factory>>::new(config, executor) },
        FullImportQueue = AuraImportQueue<Self::Block>
            { |config: &mut FactoryFullConfiguration<Self> , client: Arc<FullClient<Self>>, select_chain: Self::SelectChain| {
                let slot_duration = SlotDuration::get_or_compute(&*client)?;
                let (block_import, link_half) =
                    grandpa::block_import::<_, _, _, RuntimeApi, FullClient<Self>, _>(
                        client.clone(), client.clone(), select_chain
                    )?;
                let block_import = Arc::new(block_import);
                let justification_import = block_import.clone();

                config.custom.grandpa_import_setup = Some((block_import.clone(), link_half));

                import_queue::<_, _, _, ed25519::Pair>(
                    slot_duration,
                    block_import,
                    Some(justification_import),
                    client,
                    NothingExtra,
                    config.custom.inherent_data_providers.clone(),
                ).map_err(Into::into)
            }},
        LightImportQueue = AuraImportQueue<Self::Block>
            { |config: &FactoryFullConfiguration<Self>, client: Arc<LightClient<Self>>| {
                import_queue::<_, _, _, ed25519::Pair>(
                    SlotDuration::get_or_compute(&*client)?,
                    client.clone(),
                    None,
                    client,
                    NothingExtra,
                    config.custom.inherent_data_providers.clone(),
                ).map_err(Into::into)
            }
        },
        SelectChain = LongestChain<FullBackend<Self>, Self::Block>
            { |config: &FactoryFullConfiguration<Self>, client: Arc<FullClient<Self>>| {
                Ok(LongestChain::new(
                    client.backend().clone(),
                    client.import_lock()
                ))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "rhd")]
    fn test_sync() {
        use client::{BlockOrigin, ImportBlock};
        use {service_test, Factory};

        let alice: Arc<ed25519::Pair> = Arc::new(Keyring::Alice.into());
        let bob: Arc<ed25519::Pair> = Arc::new(Keyring::Bob.into());
        let validators = vec![alice.public().0.into(), bob.public().0.into()];
        let keys: Vec<&ed25519::Pair> = vec![&*alice, &*bob];
        let dummy_runtime = ::tokio::runtime::Runtime::new().unwrap();
        let block_factory = |service: &<Factory as service::ServiceFactory>::FullService| {
            let block_id = BlockId::number(service.client().info().unwrap().chain.best_number);
            let parent_header = service.client().header(&block_id).unwrap().unwrap();
            let consensus_net = ConsensusNetwork::new(service.network(), service.client().clone());
            let proposer_factory = consensus::ProposerFactory {
                client: service.client().clone(),
                transaction_pool: service.transaction_pool().clone(),
                network: consensus_net,
                force_delay: 0,
                handle: dummy_runtime.executor(),
            };
            let (proposer, _, _) = proposer_factory
                .init(&parent_header, &validators, alice.clone())
                .unwrap();
            let block = proposer.propose().expect("Error making test block");
            ImportBlock {
                origin: BlockOrigin::File,
                justification: Vec::new(),
                internal_justification: Vec::new(),
                finalized: true,
                body: Some(block.extrinsics),
                header: block.header,
                auxiliary: Vec::new(),
            }
        };
        let extrinsic_factory = |service: &<Factory as service::ServiceFactory>::FullService| {
            let payload = (
                0,
                Call::Balances(BalancesCall::transfer(
                    RawAddress::Id(bob.public().0.into()),
                    69.into(),
                )),
                Era::immortal(),
                service.client().genesis_hash(),
            );
            let signature = alice.sign(&payload.encode()).into();
            let id = alice.public().0.into();
            let xt = UncheckedExtrinsic {
                signature: Some((RawAddress::Id(id), signature, payload.0, Era::immortal())),
                function: payload.1,
            }
            .encode();
            let v: Vec<u8> = Decode::decode(&mut xt.as_slice()).unwrap();
            OpaqueExtrinsic(v)
        };
        service_test::sync::<Factory, _, _>(
            chain_spec::integration_test_config(),
            block_factory,
            extrinsic_factory,
        );
    }

}
