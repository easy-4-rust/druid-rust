use k8s_openapi::api::core::v1::{Pod, Service};
use kube::api::{Api, ListParams};
use kube::config::{Config, KubeConfigOptions, Kubeconfig};
use kube::{Client, ResourceExt};

use crate::model::ServiceNode;

use super::{K8sDiscoveryProvider, KubeRsDiscoveryError};

/// 基于官方 Rust Kubernetes 生态 `kube` 的生产发现实现。
#[derive(Clone, Copy, Debug, Default)]
pub struct KubeRsDiscoveryProvider;

#[async_trait::async_trait]
impl K8sDiscoveryProvider for KubeRsDiscoveryProvider {
    async fn service_nodes(
        &self,
        service_names: &[String],
        kube_config_file_path: &str,
        namespace: &str,
    ) -> Result<Vec<ServiceNode>, Box<dyn std::error::Error + Send + Sync>> {
        let path = kube_config_file_path.to_owned();
        let kubeconfig = tokio::task::spawn_blocking(move || Kubeconfig::read_from(path)).await??;
        let config =
            Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default()).await?;
        let client = Client::try_from(config)?;
        discover_nodes(client, service_names, namespace)
            .await
            .map_err(Into::into)
    }
}

async fn discover_nodes(
    client: Client,
    service_names: &[String],
    namespace: &str,
) -> Result<Vec<ServiceNode>, KubeRsDiscoveryError> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let services: Api<Service> = Api::namespaced(client, namespace);
    let pod_list = pods.list(&ListParams::default()).await?;
    let mut nodes = Vec::new();

    for service_name in service_names {
        let service = services.get(service_name).await?;
        let port = service
            .spec
            .as_ref()
            .and_then(|spec| spec.ports.as_ref())
            .into_iter()
            .flatten()
            .find(|port| {
                port.name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(service_name))
            })
            .map(|port| port.port)
            .ok_or_else(|| KubeRsDiscoveryError::MissingServicePort {
                service_name: service_name.clone(),
            })?;
        let port = u32::try_from(port).map_err(|_| KubeRsDiscoveryError::InvalidPort {
            service_name: service_name.clone(),
            port,
        })?;

        for pod in pod_list
            .items
            .iter()
            .filter(|pod| pod.name_any().starts_with(service_name))
        {
            let id =
                pod.metadata
                    .uid
                    .clone()
                    .ok_or_else(|| KubeRsDiscoveryError::MissingPodField {
                        service_name: service_name.clone(),
                        field: "metadata.uid",
                    })?;
            let address = pod
                .status
                .as_ref()
                .and_then(|status| status.pod_ip.clone())
                .ok_or_else(|| KubeRsDiscoveryError::MissingPodField {
                    service_name: service_name.clone(),
                    field: "status.podIP",
                })?;
            nodes.push(ServiceNode {
                id,
                port,
                address,
                service_name: service_name.clone(),
            });
        }
    }
    Ok(nodes)
}
