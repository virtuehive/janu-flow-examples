//
// Copyright (c) 2017, 2021 ADLINK Technology Inc.
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ADLINK janu team, <janu@adlink-labs.tech>
//

use async_trait::async_trait;
use janu_flow::async_std::sync::Arc;
use janu_flow::Configuration;
use janu_flow::{types::ZFResult, zf_empty_state, Context, Data, Node, Source, State};
use janu_flow_example_types::ZFUsize;

#[derive(Debug)]
struct ExampleRandomSource;

impl Node for ExampleRandomSource {
    fn initialize(&self, _configuration: &Option<Configuration>) -> ZFResult<State> {
        zf_empty_state!()
    }

    fn finalize(&self, _state: &mut State) -> ZFResult<()> {
        Ok(())
    }
}

#[async_trait]
impl Source for ExampleRandomSource {
    async fn run(&self, _context: &mut Context, _state: &mut State) -> ZFResult<Data> {
        janu_flow::async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        Ok(Data::from::<ZFUsize>(ZFUsize(rand::random::<usize>())))
    }
}

// Also generated by macro
janu_flow::export_source!(register);

fn register() -> ZFResult<Arc<dyn Source>> {
    Ok(Arc::new(ExampleRandomSource) as Arc<dyn Source>)
}
