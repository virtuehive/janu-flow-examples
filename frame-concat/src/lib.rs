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

use async_std::sync::{Arc, Mutex};
use std::collections::HashMap;
use janu_flow::{
    default_input_rule, default_output_rule, janu_flow_derive::ZFState, zf_spin_lock, Data, Node,
    Operator, State, ZFError, ZFResult,
};
use janu_flow::{Configuration, LocalDeadlineMiss};

use opencv::core;

static INPUT1: &str = "Frame1";
static INPUT2: &str = "Frame2";
static OUTPUT: &str = "Frame";

#[derive(ZFState, Clone)]
struct FrameConcatState {
    pub encode_options: Arc<Mutex<opencv::types::VectorOfi32>>,
}

// because of opencv
impl std::fmt::Debug for FrameConcatState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ConcatState:...",)
    }
}

impl FrameConcatState {
    fn new() -> Self {
        Self {
            encode_options: Arc::new(Mutex::new(opencv::types::VectorOfi32::new())),
        }
    }
}

struct FrameConcat;

impl Node for FrameConcat {
    fn initialize(&self, _configuration: &Option<Configuration>) -> ZFResult<State> {
        Ok(State::from(FrameConcatState::new()))
    }

    fn finalize(&self, _state: &mut State) -> ZFResult<()> {
        Ok(())
    }
}

impl Operator for FrameConcat {
    fn input_rule(
        &self,
        _context: &mut janu_flow::Context,
        state: &mut State,
        tokens: &mut HashMap<janu_flow::PortId, janu_flow::Token>,
    ) -> ZFResult<bool> {
        default_input_rule(state, tokens)
    }

    fn run(
        &self,
        _context: &mut janu_flow::Context,
        dyn_state: &mut State,
        inputs: &mut HashMap<janu_flow::PortId, janu_flow::runtime::message::DataMessage>,
    ) -> ZFResult<HashMap<janu_flow::PortId, Data>> {
        let mut results: HashMap<janu_flow::PortId, Data> = HashMap::new();

        let state = dyn_state.try_get::<FrameConcatState>()?;
        let encode_options = zf_spin_lock!(state.encode_options);

        let mut input_frame1 = inputs
            .remove(INPUT1)
            .ok_or_else(|| ZFError::InvalidData("No data".to_string()))?;
        let frame1 = input_frame1
            .get_inner_data()
            .try_as_bytes()?
            .as_ref()
            .clone();

        let mut input_frame2 = inputs
            .remove(INPUT2)
            .ok_or_else(|| ZFError::InvalidData("No data".to_string()))?;
        let frame2 = input_frame2
            .get_inner_data()
            .try_as_bytes()?
            .as_ref()
            .clone();

        // Decode Image
        let frame1 = opencv::imgcodecs::imdecode(
            &opencv::types::VectorOfu8::from_iter(frame1),
            opencv::imgcodecs::IMREAD_COLOR,
        )
        .unwrap();

        let frame2 = opencv::imgcodecs::imdecode(
            &opencv::types::VectorOfu8::from_iter(frame2),
            opencv::imgcodecs::IMREAD_COLOR,
        )
        .unwrap();

        let mut frame = core::Mat::default();

        // concat frames
        core::vconcat2(&frame1, &frame2, &mut frame).unwrap();

        let mut buf = opencv::types::VectorOfu8::new();
        opencv::imgcodecs::imencode(".jpg", &frame, &mut buf, &encode_options).unwrap();

        results.insert(OUTPUT.into(), Data::from_bytes(buf.into()));

        Ok(results)
    }

    fn output_rule(
        &self,
        _context: &mut janu_flow::Context,
        state: &mut State,
        outputs: HashMap<janu_flow::PortId, Data>,
        _deadlinemiss: Option<LocalDeadlineMiss>,
    ) -> ZFResult<HashMap<janu_flow::PortId, janu_flow::NodeOutput>> {
        default_output_rule(state, outputs)
    }
}

janu_flow::export_operator!(register);

fn register() -> ZFResult<Arc<dyn Operator>> {
    Ok(Arc::new(FrameConcat) as Arc<dyn Operator>)
}
