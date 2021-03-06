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

//! Substrate CLI library.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

pub use cli::error;
pub mod chain_spec;
mod params;
mod service;

use crate::params::VendorCmd;
pub use cli::{IntoExit, NoCustom, VersionInfo};
use log::info;
use std::ops::Deref;
use substrate_service::{Roles as ServiceRoles, ServiceFactory};
use tokio::prelude::Future;
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

/// The chain specification option.
#[derive(Clone, Debug)]
pub enum ChainSpec {
    /// Whatever the current runtime is, with just Alice as an auth.
    Development,
    /// Whatever the current runtime is, with simple Alice/Bob auths.
    LocalTestnet,
    /// Ladder testnet.
    LadderTestnet,
}

/// Get a chain config from a spec setting.
impl ChainSpec {
    pub(crate) fn load(self) -> Result<chain_spec::ChainSpec, String> {
        Ok(match self {
            ChainSpec::Development => chain_spec::development_config(),
            ChainSpec::LocalTestnet => chain_spec::local_testnet_config(),
            ChainSpec::LadderTestnet => chain_spec::ladder_testnet_config(),
        })
    }

    pub(crate) fn from(s: &str) -> Option<Self> {
        match s {
            "dev" => Some(ChainSpec::Development),
            "local" => Some(ChainSpec::LocalTestnet),
            "" | "ladder" => Some(ChainSpec::LadderTestnet),
            _ => None,
        }
    }
}

fn load_spec(id: &str) -> Result<Option<chain_spec::ChainSpec>, String> {
    Ok(match ChainSpec::from(id) {
        Some(spec) => Some(spec.load()?),
        None => None,
    })
}

/// Parse command line arguments into service configuration.
pub fn run<I, T, E>(args: I, exit: E, version: cli::VersionInfo) -> error::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
    E: IntoExit,
{
    cli::parse_and_execute::<service::Factory, VendorCmd, VendorCmd, _, _, _, _, _>(
        load_spec,
        &version,
        "ladder-node",
        args,
        exit,
        |exit, _cli_args, custom_args, mut config| {
            info!("{}", version.name);
            info!("  version {}", config.full_version());
            info!("  by Ladder Network, 2019");
            info!("Chain specification: {}", config.chain_spec.name());
            info!("Node name: {}", config.name);
            info!("Roles: {:?}", config.roles);
            config.custom.custom_args = custom_args;
            let runtime = RuntimeBuilder::new()
                .name_prefix("main-tokio-")
                .build()
                .map_err(|e| format!("{:?}", e))?;
            let executor = runtime.executor();
            match config.roles {
                ServiceRoles::LIGHT => run_until_exit(
                    runtime,
                    service::Factory::new_light(config, executor)
                        .map_err(|e| format!("{:?}", e))?,
                    exit,
                ),
                _ => run_until_exit(
                    runtime,
                    service::Factory::new_full(config, executor).map_err(|e| format!("{:?}", e))?,
                    exit,
                ),
            }
            .map_err(|e| format!("{:?}", e))
        },
    )
    .map_err(Into::into)
    .map(|_| ())
}

fn run_until_exit<T, C, E>(mut runtime: Runtime, service: T, e: E) -> error::Result<()>
where
    T: Deref<Target = substrate_service::Service<C>>,
    C: substrate_service::Components,
    E: IntoExit,
{
    let (exit_send, exit) = exit_future::signal();

    let executor = runtime.executor();
    cli::informant::start(&service, exit.clone(), executor.clone());

    let _ = runtime.block_on(e.into_exit());
    exit_send.fire();

    // we eagerly drop the service so that the internal exit future is fired,
    // but we need to keep holding a reference to the global telemetry guard
    let _telemetry = service.telemetry();
    drop(service);

    // TODO [andre]: timeout this future #1318
    let _ = runtime.shutdown_on_idle().wait();

    Ok(())
}
