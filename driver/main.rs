use std::cmp::max;
use std::fs;
use std::time::Duration;

use anyhow::anyhow;
use clap::Parser;
use k8s_openapi::api::core::v1 as corev1;
use kube::api::{
    DynamicObject,
    Patch,
    PatchParams,
};
use kube::ResourceExt;
use serde_json::json;
use simkube::jsonutils;
use simkube::k8s::{
    add_common_metadata,
    build_global_object_meta,
    prefixed_ns,
    ApiSet,
    GVK,
};
use simkube::macros::*;
use simkube::prelude::*;
use simkube::store::TraceStore;
use tokio::time::sleep;
use tracing::*;

#[derive(Parser, Debug)]
struct Options {
    #[arg(long)]
    sim_name: String,

    #[arg(long)]
    sim_root: String,

    #[arg(long)]
    sim_namespace_prefix: String,

    #[arg(long)]
    trace_path: String,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

fn build_virtual_ns(sim_name: &str, ns_name: &str, sim_root: &SimulationRoot) -> anyhow::Result<corev1::Namespace> {
    let mut ns = corev1::Namespace {
        metadata: build_global_object_meta(ns_name, sim_name, sim_root)?,
        ..Default::default()
    };
    klabel_insert!(ns, VIRTUAL_LABEL_KEY = "true");

    Ok(ns)
}

fn build_virtual_obj(
    obj: &DynamicObject,
    vns_name: &str,
    sim_name: &str,
    root: &SimulationRoot,
    config: &TracerConfig,
) -> anyhow::Result<DynamicObject> {
    let mut vobj = obj.clone();

    vobj.metadata.namespace = Some(vns_name.into());
    klabel_insert!(vobj, VIRTUAL_LABEL_KEY = "true");

    let gvk = GVK::from_dynamic_obj(obj)?;
    let psp = &config.tracked_objects[&gvk].pod_spec_path;

    jsonutils::patch_ext::add(psp, "nodeSelector", &json!({"type": "virtual"}), &mut vobj.data, true)?;
    jsonutils::patch_ext::add(psp, "tolerations", &json!([]), &mut vobj.data, false)?;
    jsonutils::patch_ext::add(
        &format!("{}/tolerations", psp),
        "-",
        &json!({"key": "simkube.io/virtual-node", "value": "true"}),
        &mut vobj.data,
        true,
    )?;
    jsonutils::patch_ext::remove(psp, "status", &mut vobj.data)?;
    add_common_metadata(sim_name, root, &mut vobj.metadata)?;

    Ok(vobj)
}

async fn run(args: &Options) -> EmptyResult {
    info!("Simulation driver starting");

    let trace_data = fs::read(&args.trace_path)?;
    let trace_store = TraceStore::import(trace_data)?;

    let client = kube::Client::try_default().await?;
    let mut apiset = ApiSet::new(client.clone());
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(client.clone());
    let ns_api: kube::Api<corev1::Namespace> = kube::Api::all(client.clone());

    let root = roots_api.get(&args.sim_root).await?;

    let mut sim_ts = trace_store.start_ts().ok_or(anyhow!("no trace data"))?;
    for (evt, next_ts) in trace_store.iter() {
        for obj in evt.applied_objs {
            let gvk = GVK::from_dynamic_obj(&obj)?;
            let vns_name = prefixed_ns(&args.sim_namespace_prefix, &obj);
            let vobj = build_virtual_obj(&obj, &vns_name, &args.sim_name, &root, trace_store.config())?;

            let vns = build_virtual_ns(&args.sim_name, &vns_name, &root)?;
            ns_api.create(&Default::default(), &vns).await?;

            info!("applying object {:?}", vobj);
            apiset
                .namespaced_api_for(gvk, vns_name)
                .await?
                .patch(&vobj.name_any(), &PatchParams::apply("simkube"), &Patch::Apply(&vobj))
                .await?;
        }

        for obj in evt.deleted_objs {
            info!("deleting pod {}", obj.name_any());
            let gvk = GVK::from_dynamic_obj(&obj)?;
            let vns_name = prefixed_ns(&args.sim_namespace_prefix, &obj);
            apiset
                .namespaced_api_for(gvk, vns_name)
                .await?
                .delete(&obj.name_any(), &Default::default())
                .await?;
        }

        if let Some(ts) = next_ts {
            let sleep_duration = max(0, ts - sim_ts);
            sim_ts = ts;
            info!("next event happens in {} seconds, sleeping", sleep_duration);
            sleep(Duration::from_secs(sleep_duration as u64)).await;
        }
    }

    info!("trace over, cleaning up");
    roots_api.delete(&root.name_any(), &Default::default()).await?;
    info!("simulation complete!");

    Ok(())
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&args.verbosity)?;
    run(&args).await?;
    Ok(())
}
