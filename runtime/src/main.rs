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

// TODO: this should become a deamon.

use std::convert::TryFrom;
use std::fs::{File, *};
use std::io::Write;
use std::path::Path;
use structopt::StructOpt;
use janu_flow::async_std::sync::Arc;
use janu_flow::runtime::dataflow::loader::{Loader, LoaderConfig};
use janu_flow::runtime::RuntimeContext;

#[derive(Debug, StructOpt)]
#[structopt(name = "dpn")]
struct Opt {
    #[structopt(short = "g", long = "graph-file")]
    graph_file: String,
    #[structopt(short = "o", long = "out-file", default_value = "output.dot")]
    _outfile: String,
    #[structopt(short = "l", long = "loader_config")]
    loader_config: Option<String>,
    #[structopt(short = "r", long = "runtime")]
    runtime: String,
}

fn _write_record_to_file(
    record: janu_flow::model::dataflow::record::DataFlowRecord,
    filename: &str,
) {
    let path = Path::new(filename);
    let mut write_file = File::create(path).unwrap();
    write!(write_file, "{}", record.to_yaml().unwrap()).unwrap();
}

#[async_std::main]
async fn main() {
    env_logger::init();

    let opt = Opt::from_args();
    let yaml_df = read_to_string(opt.graph_file).unwrap();

    let loader_config = match opt.loader_config {
        Some(config) => {
            let yaml_conf = read_to_string(config).unwrap();
            serde_yaml::from_str::<LoaderConfig>(&yaml_conf).unwrap()
        }
        None => LoaderConfig { extensions: vec![] },
    };

    let session = Arc::new(janu::open(janu::config::Config::default()).await.unwrap());
    let hlc = async_std::sync::Arc::new(uhlc::HLC::default());
    let loader = Arc::new(Loader::new(loader_config));

    let ctx = RuntimeContext {
        session,
        hlc,
        loader,
        runtime_name: opt.runtime.clone().into(),
        runtime_uuid: uuid::Uuid::new_v4(),
    };

    // loading the descriptor
    let df =
        janu_flow::model::dataflow::descriptor::DataFlowDescriptor::from_yaml(&yaml_df).unwrap();

    // mapping to infrastructure
    let mapped = janu_flow::runtime::map_to_infrastructure(df, &opt.runtime)
        .await
        .unwrap();

    // creating record
    let dfr =
        janu_flow::model::dataflow::record::DataFlowRecord::try_from((mapped, uuid::Uuid::nil()))
            .unwrap();

    _write_record_to_file(dfr.clone(), "computed-record.yaml");

    // creating dataflow
    let dataflow = janu_flow::runtime::dataflow::Dataflow::try_new(ctx.clone(), dfr).unwrap();

    // instantiating
    let mut instance =
        janu_flow::runtime::dataflow::instance::DataflowInstance::try_instantiate(dataflow)
            .unwrap();

    let mut sinks = instance.get_sinks();
    for id in sinks.drain(..) {
        instance.start_node(&id).await.unwrap()
    }

    let mut operators = instance.get_operators();
    for id in operators.drain(..) {
        instance.start_node(&id).await.unwrap()
    }

    let mut connectors = instance.get_connectors();
    for id in connectors.drain(..) {
        instance.start_node(&id).await.unwrap()
    }

    let sources = instance.get_sources();
    for id in &sources {
        instance.start_node(id).await.unwrap()
    }

    let () = std::future::pending().await;
}
