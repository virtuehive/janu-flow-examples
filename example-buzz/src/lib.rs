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
use janu_flow::janu_flow_derive::ZFState;
use janu_flow::{
    default_input_rule, default_output_rule, export_operator, types::ZFResult, Node, NodeOutput,
    Operator, State, Token,
};
use janu_flow::{Configuration, LocalDeadlineMiss};
use janu_flow::{Context, Data, ZFError};
use janu_flow_example_types::{ZFString, ZFUsize};

struct BuzzOperator;

#[derive(Debug, ZFState)]
struct BuzzState {
    buzzword: String,
}

static LINK_ID_INPUT_INT: &str = "Int";
static LINK_ID_INPUT_STR: &str = "Str";
static LINK_ID_OUTPUT_STR: &str = "Str";

impl Operator for BuzzOperator {
    fn input_rule(
        &self,
        _context: &mut Context,
        state: &mut State,
        tokens: &mut HashMap<janu_flow::PortId, Token>,
    ) -> ZFResult<bool> {
        default_input_rule(state, tokens)
    }

    fn run(
        &self,
        _context: &mut Context,
        dyn_state: &mut State,
        inputs: &mut HashMap<janu_flow::PortId, DataMessage>,
    ) -> ZFResult<HashMap<janu_flow::PortId, Data>> {
        let mut results = HashMap::<janu_flow::PortId, Data>::with_capacity(1);

        let state = dyn_state.try_get::<BuzzState>()?;

        let mut input_fizz = inputs
            .remove(LINK_ID_INPUT_STR)
            .ok_or_else(|| ZFError::InvalidData("No data".to_string()))?;

        let fizz = input_fizz.get_inner_data().try_get::<ZFString>()?;

        let mut input_value = inputs
            .remove(LINK_ID_INPUT_INT)
            .ok_or_else(|| ZFError::InvalidData("No data".to_string()))?;

        let value = input_value.get_inner_data().try_get::<ZFUsize>()?;

        let mut buzz = fizz.clone();
        if value.0 % 3 == 0 {
            buzz.0.push_str(&state.buzzword);
        }

        results.insert(
            LINK_ID_OUTPUT_STR.into(),
            Data::from::<ZFString>(buzz.clone()),
        );

        Ok(results)
    }

    fn output_rule(
        &self,
        _context: &mut Context,
        state: &mut State,
        outputs: HashMap<janu_flow::PortId, Data>,
        _deadlinemiss: Option<LocalDeadlineMiss>,
    ) -> ZFResult<HashMap<janu_flow::PortId, NodeOutput>> {
        default_output_rule(state, outputs)
    }
}

impl Node for BuzzOperator {
    fn initialize(&self, configuration: &Option<Configuration>) -> ZFResult<State> {
        let state = match configuration {
            Some(config) => match config["buzzword"].as_str() {
                Some(buzzword) => BuzzState {
                    buzzword: buzzword.to_string(),
                },
                None => BuzzState {
                    buzzword: "Buzz".to_string(),
                },
            },
            None => BuzzState {
                buzzword: "Buzz".to_string(),
            },
        };
        Ok(State::from(state))
    }

    fn finalize(&self, _state: &mut State) -> ZFResult<()> {
        Ok(())
    }
}

export_operator!(register);

fn register() -> ZFResult<Arc<dyn Operator>> {
    Ok(Arc::new(BuzzOperator) as Arc<dyn Operator>)
}
