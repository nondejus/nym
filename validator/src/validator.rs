// Copyright 2020 Nym Technologies SA
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// use crate::config::Config;
use crate::network::rest;
use crate::network::tendermint;
use crate::services::mixmining;
use tokio::runtime::Runtime;

pub struct Validator {
    // when you re-introduce keys, check which ones you want:
    //    MixIdentityKeyPair (like 'nym-client' ) <- probably that one (after maybe renaming to just identity::KeyPair)
    //    encryption::KeyPair (like 'nym-mixnode' or 'sfw-provider')
    tendermint_abci: tendermint::Abci,
    rest_api: rest::Api,
}

impl Validator {
    pub fn new() -> Self {
        let mixmining_db = mixmining::db::MixminingDb::new();
        let mixmining_service = mixmining::Service::new(mixmining_db);

        let rest_api = rest::Api::new(mixmining_service);

        Validator {
            rest_api,

            // perhaps you might want to pass &config to the constructor
            // there to get the config.tendermint (assuming you create appropriate fields + getters)
            tendermint_abci: tendermint::Abci::new(),
        }
    }

    // TODO: Fix Tendermint startup here, see https://github.com/nymtech/nym/issues/147
    pub fn start(self) {
        let mut rt = Runtime::new().unwrap();
        rt.spawn(self.rest_api.run());
        rt.spawn(self.tendermint_abci.run());

        // TODO: this message is going to come out of order (if at all), as spawns are async, see issue above
        println!("Validator startup complete.");
        rt.block_on(blocker());
    }
}

pub async fn blocker() {} // once Tendermint unblocks us, make this block forever.
