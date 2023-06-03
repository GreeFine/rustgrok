use k8s_openapi::{
    api::networking::v1::{Ingress, IngressRule},
    serde_json::{self, json},
};
use kube::{
    api::{Api, PostParams},
    Client,
};

pub async fn expose_subdomain(new_host: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Infer the runtime environment and try to create a Kubernetes Client
    let client = Client::try_default().await?;

    // Read pods in the configured namespace into the typed interface from k8s-openapi
    let ingress: Api<Ingress> = Api::namespaced(client, "rustgrok");

    let ingress_name = "server";
    let mut current_ingress = ingress.get(ingress_name).await?;

    let rules = current_ingress
        .spec
        .as_mut()
        .unwrap()
        .rules
        .as_mut()
        .unwrap();
    if !rules.iter().any(|r| r.host.as_deref() == Some(new_host)) {
        let new_rule: IngressRule = serde_json::from_value(json!({
                "host": new_host,
                "http": {
                  "paths": [
                    {
                      "backend": {
                        "service": {
                          "name": "server",
                          "port": {
                            "number": 8080
                          }
                        }
                      },
                      "pathType": "ImplementationSpecific"
                    }
                  ]
                }
        }))
        .unwrap();
        rules.push(new_rule);
        ingress
            .replace(ingress_name, &PostParams::default(), &current_ingress)
            .await?;
    };

    Ok(())
}
