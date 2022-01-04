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
use std::{
    fs::File,
    io::{prelude::*, BufReader},
    path::Path,
};
use janu_flow::{
    default_input_rule, default_output_rule, janu_flow_derive::ZFState, zf_spin_lock, Context,
    Data, Node, NodeOutput, Operator, PortId, State, Token, ZFError, ZFResult,
};
use janu_flow::{Configuration, LocalDeadlineMiss};

use opencv::core::prelude::MatTrait;
use opencv::dnn::NetTrait;
use opencv::{core, imgproc};
use std::time::Instant;

static INPUT: &str = "Frame";
static OUTPUT: &str = "Frame";

#[derive(Debug)]
struct ObjDetection;

#[derive(ZFState, Clone)]
struct ODState {
    pub dnn: Arc<Mutex<opencv::dnn::Net>>,
    pub classes: Arc<Mutex<Vec<String>>>,
    pub encode_options: Arc<Mutex<opencv::types::VectorOfi32>>,
    pub outputs: Arc<Mutex<opencv::core::Vector<String>>>,
}

fn lines_from_file(filename: impl AsRef<Path>) -> Vec<String> {
    let file = File::open(filename).expect("no such file");
    let buf = BufReader::new(file);
    buf.lines()
        .map(|l| l.expect("Could not parse line"))
        .collect()
}

// because of opencv
impl std::fmt::Debug for ODState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "FDState:...",)
    }
}

impl ODState {
    fn new(configuration: &Option<Configuration>) -> Self {
        // Configuration is mandatory.
        let configuration = configuration.as_ref().unwrap();

        let net_cfg = configuration["neural-network"].as_str().unwrap();
        let net_weights = configuration["network-weights"].as_str().unwrap();
        let net_classes = configuration["network-classes"].as_str().unwrap();
        let classes = lines_from_file(net_classes);

        let mut net = opencv::dnn::read_net_from_darknet(net_cfg, net_weights).unwrap();
        let encode_options = opencv::types::VectorOfi32::new();

        net.set_preferable_backend(opencv::dnn::DNN_BACKEND_CUDA)
            .unwrap();
        net.set_preferable_target(opencv::dnn::DNN_TARGET_CUDA)
            .unwrap();

        let output_names = net.get_unconnected_out_layers_names().unwrap();

        Self {
            dnn: Arc::new(Mutex::new(net)),
            classes: Arc::new(Mutex::new(classes)),
            encode_options: Arc::new(Mutex::new(encode_options)),
            outputs: Arc::new(Mutex::new(output_names)),
        }
    }
}

impl Node for ObjDetection {
    fn initialize(&self, configuration: &Option<Configuration>) -> ZFResult<State> {
        Ok(State::from(ODState::new(configuration)))
    }

    fn finalize(&self, _state: &mut State) -> ZFResult<()> {
        Ok(())
    }
}

impl Operator for ObjDetection {
    fn input_rule(
        &self,
        _context: &mut Context,
        state: &mut State,
        tokens: &mut HashMap<PortId, Token>,
    ) -> ZFResult<bool> {
        default_input_rule(state, tokens)
    }

    fn run(
        &self,
        _context: &mut Context,
        dyn_state: &mut State,
        inputs: &mut HashMap<PortId, janu_flow::runtime::message::DataMessage>,
    ) -> ZFResult<HashMap<janu_flow::PortId, Data>> {
        let scale = 1.0 / 255.0;
        let mean = core::Scalar::new(0f64, 0f64, 0f64, 0f64);

        let mut results: HashMap<PortId, Data> = HashMap::with_capacity(1);

        let mut detections: opencv::types::VectorOfMat = core::Vector::new();

        let state = dyn_state.try_get::<ODState>()?;

        let mut net = zf_spin_lock!(state.dnn);
        let encode_options = zf_spin_lock!(state.encode_options);
        let classes = zf_spin_lock!(state.classes);
        let outputs = zf_spin_lock!(state.outputs);

        let mut boxes: Vec<core::Vector<core::Rect>> = vec![core::Vector::new(); classes.len()];
        let mut scores: Vec<core::Vector<f32>> = vec![core::Vector::new(); classes.len()];
        let mut indices: Vec<core::Vector<i32>> = vec![core::Vector::new(); classes.len()];

        let colors: Vec<core::Scalar> = vec![
            core::Scalar::new(0f64, 255f64, 0f64, -1f64),
            core::Scalar::new(255f64, 255f64, 0f64, -1f64),
            core::Scalar::new(0f64, 255f64, 255f64, -1f64),
            core::Scalar::new(255f64, 0f64, 0f64, -1f64),
        ];

        let mut input_value = inputs
            .remove(INPUT)
            .ok_or_else(|| ZFError::InvalidData("No data".to_string()))?;
        let data = input_value
            .get_inner_data()
            .try_as_bytes()?
            .as_ref()
            .clone();

        // Decode Image
        let mut frame = opencv::imgcodecs::imdecode(
            &opencv::types::VectorOfu8::from_iter(data),
            opencv::imgcodecs::IMREAD_COLOR,
        )
        .unwrap();

        // create blob
        let blob = opencv::dnn::blob_from_image(
            &frame,
            scale,
            core::Size {
                width: 512,
                height: 512, //416 //608
            },
            mean,
            true,
            false,
            opencv::core::CV_32F, //CV_32F
        )
        .unwrap();

        //set the input
        net.set_input(&blob, "", 1.0, core::Scalar::new(0f64, 0f64, 0f64, 0f64))
            .unwrap();

        //run the DNN
        let now = Instant::now();
        net.forward(&mut detections, &outputs).unwrap();
        let elapsed = now.elapsed().as_micros();

        // loop on the detected objects
        for obj in detections {
            let num_boxes = obj.rows();

            for i in 0..num_boxes {
                let x = obj.at_2d::<f32>(i, 0).unwrap() * frame.cols() as f32;
                let y = obj.at_2d::<f32>(i, 1).unwrap() * frame.rows() as f32;
                let width = obj.at_2d::<f32>(i, 2).unwrap() * frame.cols() as f32;
                let height = obj.at_2d::<f32>(i, 3).unwrap() * frame.rows() as f32;

                let scaled_obj = core::Rect {
                    x: (x - width / 2.0) as i32,
                    y: (y - height / 2.0) as i32,
                    width: width as i32,
                    height: height as i32,
                };

                for c in 0..classes.len() {
                    let conf = *obj.at_2d::<f32>(i, 5 + (c as i32)).unwrap();
                    if conf >= 0.4 {
                        boxes[c].push(scaled_obj);
                        scores[c].push(conf);
                    }
                }
            }
        }

        //remove duplicates
        for c in 0..classes.len() {
            opencv::dnn::nms_boxes(&boxes[c], &scores[c], 0.0, 0.4, &mut indices[c], 1.0, 0)
                .unwrap();
        }

        let mut detected = 0;

        // add boxes with score
        for c in 0..classes.len() {
            for i in &indices[c] {
                let rect = boxes[c].get(i as usize).unwrap();
                let score = scores[c].get(i as usize).unwrap();

                let color = colors[c % 4];

                imgproc::rectangle(
                    &mut frame, rect, color, //green
                    2, 1, 0,
                )
                .unwrap();

                let label = format!("{}: {}", classes[c], score);
                let mut baseline = 0;
                imgproc::get_text_size(
                    &label,
                    opencv::imgproc::FONT_HERSHEY_COMPLEX_SMALL,
                    1.0,
                    1,
                    &mut baseline,
                )
                .unwrap();

                imgproc::put_text(
                    &mut frame,
                    &label,
                    core::Point_::new(rect.x, rect.y - baseline - 5),
                    opencv::imgproc::FONT_HERSHEY_COMPLEX_SMALL,
                    1.0,
                    color, //black
                    2,
                    8,
                    false,
                )
                .unwrap();

                detected += 1;
            }
        }

        // add label to frame with info
        let label = format!(
            "DNN Inference Time: {} us - Detected: {}",
            elapsed, detected
        );
        let mut baseline = 0;

        let bg_size = imgproc::get_text_size(
            &label,
            opencv::imgproc::FONT_HERSHEY_COMPLEX_SMALL,
            1.0,
            1,
            &mut baseline,
        )
        .unwrap();
        let rect = core::Rect {
            x: 0,
            y: 0,
            width: bg_size.width,
            height: bg_size.height + 10,
        };

        imgproc::rectangle(
            &mut frame,
            rect,
            core::Scalar::new(0f64, 0f64, 0f64, -1f64), //black
            imgproc::FILLED,
            1,
            0,
        )
        .unwrap();
        imgproc::put_text(
            &mut frame,
            &label,
            core::Point_::new(0, bg_size.height + 5),
            opencv::imgproc::FONT_HERSHEY_COMPLEX_SMALL,
            1.0,
            core::Scalar::new(255f64, 255f64, 0f64, -1f64), //yellow
            2,
            8,
            false,
        )
        .unwrap();

        // encode and send
        let mut buf = opencv::types::VectorOfu8::new();
        opencv::imgcodecs::imencode(".jpg", &frame, &mut buf, &encode_options).unwrap();

        results.insert(OUTPUT.into(), Data::from_bytes(buf.into()));

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

janu_flow::export_operator!(register);

fn register() -> ZFResult<Arc<dyn janu_flow::Operator>> {
    Ok(Arc::new(ObjDetection) as Arc<dyn janu_flow::Operator>)
}
