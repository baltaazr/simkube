use k8s_openapi::api::admissionregistration::v1 as admissionv1;
use k8s_openapi::api::batch::v1 as batchv1;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::ResourceExt;
use reqwest::Url;
use simkube::k8s::{
    build_global_object_meta,
    build_object_meta,
};
use simkube::macros::*;
use simkube::prelude::*;
use simkube::store::storage;

use super::trace::get_local_trace_volume;

const WEBHOOK_NAME: &str = "mutatepods.simkube.io";

pub(super) fn sim_root_name(sim_name: &str) -> String {
    format!("sk-{}-root", sim_name)
}

pub(super) fn build_simulation_root(name: &str, sim_name: &str, owner: &Simulation) -> anyhow::Result<SimulationRoot> {
    Ok(SimulationRoot {
        metadata: build_global_object_meta(name, sim_name, owner)?,
        spec: SimulationRootSpec {},
    })
}

pub(super) fn build_driver_namespace(
    driver_ns: &str,
    sim_name: &str,
    owner: &Simulation,
) -> anyhow::Result<corev1::Namespace> {
    Ok(corev1::Namespace {
        metadata: build_global_object_meta(driver_ns, sim_name, owner)?,
        ..Default::default()
    })
}

pub(super) fn mutating_webhook_config_name(sim_name: &str) -> String {
    format!("sk-{}-mutatepods", sim_name)
}

pub(super) fn build_mutating_webhook(
    name: &str,
    driver_ns_name: &str,
    driver_service_name: &str,
    driver_port: i32,
    sim_name: &str,
    owner: &SimulationRoot,
) -> anyhow::Result<admissionv1::MutatingWebhookConfiguration> {
    Ok(admissionv1::MutatingWebhookConfiguration {
        metadata: build_global_object_meta(name, sim_name, owner)?,
        webhooks: Some(vec![admissionv1::MutatingWebhook {
            admission_review_versions: vec!["v1".into()],
            client_config: admissionv1::WebhookClientConfig {
                service: Some(admissionv1::ServiceReference {
                    namespace: driver_ns_name.into(),
                    name: driver_service_name.into(),
                    port: Some(driver_port),
                    ..Default::default()
                }),
                ..Default::default()
            },
            failure_policy: Some("Ignore".into()),
            name: WEBHOOK_NAME.into(),
            side_effects: "None".into(),
            rules: Some(vec![admissionv1::RuleWithOperations {
                api_groups: Some(vec!["".into()]),
                api_versions: Some(vec!["v1".into()]),
                operations: Some(vec!["CREATE".into()]),
                resources: Some(vec!["pods".into()]),
                scope: Some("Namespaced".into()),
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    })
}

pub(super) fn driver_service_name(sim_name: &str) -> String {
    format!("sk-{}-driver-svc", sim_name)
}

pub(super) fn build_driver_service(
    namespace: &str,
    name: &str,
    port: i32,
    sim_name: &str,
    owner: &SimulationRoot,
) -> anyhow::Result<corev1::Service> {
    Ok(corev1::Service {
        metadata: build_object_meta(namespace, name, sim_name, owner)?,
        spec: Some(corev1::ServiceSpec {
            ports: Some(vec![corev1::ServicePort {
                port,
                target_port: Some(IntOrString::Int(port)),
                ..Default::default()
            }]),
            selector: klabel!("app" = "sk-driver"),
            ..Default::default()
        }),
        ..Default::default()
    })
}

pub(super) fn sim_driver_name(sim_name: &str) -> String {
    format!("sk-{}-driver", sim_name)
}

pub(super) fn build_driver_job(
    namespace: &str,
    name: &str,
    driver_image: &str,
    trace_path: &str,
    sim_service_account_name: &str,
    sim_name: &str,
    root: &SimulationRoot,
    owner: &Simulation,
) -> anyhow::Result<batchv1::Job> {
    let trace_url = Url::parse(trace_path)?;
    let (trace_vm, trace_volume, mount_path) = match storage::get_scheme(&trace_url)? {
        storage::Scheme::AmazonS3 => todo!(),
        storage::Scheme::Local => get_local_trace_volume(&trace_url)?,
    };

    Ok(batchv1::Job {
        metadata: build_object_meta(namespace, name, sim_name, owner)?,
        spec: Some(batchv1::JobSpec {
            backoff_limit: Some(1),
            template: corev1::PodTemplateSpec {
                spec: Some(corev1::PodSpec {
                    containers: vec![corev1::Container {
                        name: "driver".into(),
                        command: Some(vec!["/sk-driver".into()]),
                        args: Some(vec![
                            "--trace-path".into(),
                            mount_path,
                            "--sim-namespace-prefix".into(),
                            "virtual".into(),
                            "--sim-root".into(),
                            root.name_any(),
                            "--sim-name".into(),
                            sim_name.into(),
                        ]),
                        image: Some(driver_image.into()),
                        volume_mounts: Some(vec![trace_vm]),
                        ..Default::default()
                    }],
                    restart_policy: Some("Never".into()),
                    volumes: Some(vec![trace_volume]),
                    service_account: Some(sim_service_account_name.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    })
}
