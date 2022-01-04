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

use std::collections::HashMap;
use janu_flow::async_std::sync::Arc;
use janu_flow::runtime::message::DataMessage;
use janu_flow::Configuration;
use janu_flow::LocalDeadlineMiss;
use janu_flow::Token;
use janu_flow::{
    default_input_rule, default_output_rule, export_operator, types::ZFResult, zf_empty_state,
    Node, NodeOutput, Operator, State, ZFError,
};
use janu_flow::{Context, Data, PortId};
use janu_flow_example_types::{ZFString, ZFUsize};

struct FizzOperator;

static LINK_ID_INPUT_INT: &str = "Int";
static LINK_ID_OUTPUT_INT: &str = "Int";
static LINK_ID_OUTPUT_STR: &str = "Str";

impl Node for FizzOperator {
    fn initialize(&self, _configuration: &Option<Configuration>) -> ZFResult<State> {
        zf_empty_state!()
    }

    fn finalize(&self, _state: &mut State) -> ZFResult<()> {
        Ok(())
    }
}

impl Operator for FizzOperator {
    fn input_rule(
        &self,
        _context: &mut Context,
        state: &mut State,
        inputs: &mut HashMap<PortId, Token>,
    ) -> ZFResult<bool> {
        default_input_rule(state, inputs)
    }

    fn run(
        &self,
        _context: &mut Context,
        _state: &mut State,
        inputs: &mut HashMap<PortId, DataMessage>,
    ) -> ZFResult<HashMap<janu_flow::PortId, Data>> {
        let mut results = HashMap::<PortId, Data>::with_capacity(2);

        let mut fizz = ZFString::from("");

        let mut input_value = inputs
            .remove(LINK_ID_INPUT_INT)
            .ok_or_else(|| ZFError::InvalidData("No data".to_string()))?;

        let zfusize = input_value.get_inner_data().try_get::<ZFUsize>()?;

        if zfusize.0 % 2 == 0 {
            fizz = ZFString::from("Fizz");
        }

        results.insert(
            LINK_ID_OUTPUT_INT.into(),
            Data::from::<ZFUsize>(zfusize.clone()),
        );

        results.insert(LINK_ID_OUTPUT_STR.into(), Data::from::<ZFString>(fizz));

        Ok(results)
    }

    fn output_rule(
        &self,
        _context: &mut Context,
        state: &mut State,
        outputs: HashMap<PortId, Data>,
        _deadlinemiss: Option<LocalDeadlineMiss>,
    ) -> ZFResult<HashMap<janu_flow::PortId, NodeOutput>> {
        default_output_rule(state, outputs)
    }
}

export_operator!(register);

fn register() -> ZFResult<Arc<dyn Operator>> {
    Ok(Arc::new(FizzOperator) as Arc<dyn Operator>)
}
