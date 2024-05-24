use simkube::metrics::api::prometheus::PrometheusRemoteWrite;
use simkube::prelude::*;

#[derive(clap::Args)]
#[command(disable_help_flag = true, disable_version_flag = true)]
pub struct Args {
    #[arg(long_help = "duration of the simulation", allow_hyphen_values = true)]
    pub duration: Option<String>,

    #[arg(short, long, long_help = "name of the simulation to run")]
    pub name: String,

    #[arg(
        short = 'N',
        long,
        long_help = "number of repetitions of the simulation to run",
        default_value = "1"
    )]
    pub repetitions: i32,

    #[arg(
        long,
        short = 'f',
        long_help = "location of the trace file for sk-driver to read",
        default_value = "file:///data/trace"
    )]
    pub trace_file: String,

    #[arg(long, long_help = "namespace to launch sk-driver in", default_value = "simkube")]
    pub driver_namespace: String,

    #[arg(
        long,
        long_help = "don't spawn Prometheus pod before running sim",
        help_heading = "Metrics"
    )]
    pub disable_metrics: bool,

    #[arg(
        long,
        long_help = "namespace to launch monitoring utilities in",
        default_value = DEFAULT_METRICS_NS,
        help_heading = "Metrics",
    )]
    pub metrics_namespace: String,

    #[arg(
        long,
        long_help = "service account with monitoring permissions",
        default_value = DEFAULT_METRICS_SVC_ACCOUNT,
        help_heading = "Metrics",
    )]
    pub metrics_service_account: String,

    #[arg(
        long,
        long_help = "comma-separated list of namespaces containing pod monitor configs",
        value_delimiter = ',',
        default_value = "monitoring-hd",
        help_heading = "Metrics"
    )]
    pub metrics_pod_monitor_namespaces: Option<Vec<String>>,

    #[arg(
        long,
        long_help = "comma-separated list of pod monitor config names\n\
            (if empty, uses all pod monitor configs in metrics_pod_monitor_namespaces)",
        value_delimiter = ',',
        help_heading = "Metrics"
    )]
    pub metrics_pod_monitor_names: Option<Vec<String>>,

    #[arg(
        long,
        long_help = "comma-separated list of namespaces containing service monitor configs",
        value_delimiter = ',',
        default_value = "monitoring-hd",
        help_heading = "Metrics"
    )]
    pub metrics_service_monitor_namespaces: Option<Vec<String>>,

    #[arg(
        long,
        long_help = "comma-separated list of service monitor config names\n\
            (if empty, uses all pod monitor configs in metrics_service_monitor_namespaces)",
        value_delimiter = ',',
        help_heading = "Metrics"
    )]
    pub metrics_service_monitor_names: Option<Vec<String>>,

    #[arg(long, long_help = "number of prometheus shards to run", help_heading = "Metrics")]
    pub prometheus_shards: Option<i32>,

    #[arg(long, long_help = "address for remote write endpoint", help_heading = "Metrics")]
    pub remote_write_endpoint: Option<String>,

    // We override help and version here so that it shows up in its own help group at the bottom
    // See https://github.com/clap-rs/clap/issues/4367 and https://github.com/clap-rs/clap/issues/4831
    // for more details.
    #[arg(short, long, long_help="Print help (see a summary with '-h')", action = clap::ArgAction::Help, help_heading = "Help")]
    pub help: (),

    #[arg(short='V', long, long_help="Print version", action = clap::ArgAction::Version, help_heading = "Help")]
    pub version: (),
}

pub async fn cmd(args: &Args) -> EmptyResult {
    println!("running simulation {}...", args.name);

    let metrics_config = (!args.disable_metrics).then_some(SimulationMetricsConfig {
        namespace: Some(args.metrics_namespace.clone()),
        service_account: Some(args.metrics_service_account.clone()),
        pod_monitor_namespaces: args.metrics_pod_monitor_namespaces.clone(),
        pod_monitor_names: args.metrics_pod_monitor_names.clone(),
        service_monitor_namespaces: args.metrics_service_monitor_namespaces.clone(),
        service_monitor_names: args.metrics_service_monitor_names.clone(),
        prometheus_shards: args.prometheus_shards,
        remote_write_configs: args
            .remote_write_endpoint
            .clone()
            .map_or(vec![], |url| vec![PrometheusRemoteWrite { url, ..Default::default() }]),
    });

    let sim = Simulation::new(
        &args.name,
        SimulationSpec {
            driver_namespace: args.driver_namespace.clone(),
            duration: args.duration.clone(),
            metrics_config,
            repetitions: Some(args.repetitions),
            trace_path: args.trace_file.clone(),
            hooks: None,
        },
    );
    let client = kube::Client::try_default().await?;
    let sim_api = kube::Api::<Simulation>::all(client.clone());

    sim_api.create(&Default::default(), &sim).await?;

    Ok(())
}
