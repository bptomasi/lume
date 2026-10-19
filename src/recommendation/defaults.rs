use crate::model::{Interest, RecommendationProfile};

impl Default for RecommendationProfile {
    fn default() -> Self {
        Self {
            interests: INTERESTS
                .iter()
                .copied()
                .map(|(name, weight, query)| Interest {
                    name: name.into(),
                    weight,
                    query: query.into(),
                })
                .collect(),
            exclude: String::new(),
            minutes: None,
            include_read: false,
            sources: Default::default(),
        }
    }
}

const INTERESTS: &[(&str, u8, &str)] = &[
    (
        "Kubernetes internals",
        8,
        concat!(
            "kubernetes OR kubelet OR etcd OR \"kube proxy\" OR ",
            "\"container runtime\"",
        ),
    ),
    (
        "Networking and dataplanes",
        9,
        concat!(
            "networking OR cilium OR CNI OR BGP OR conntrack OR ",
            "\"connection tracking\" OR \"network policy\" OR ",
            "dataplane OR XDP",
        ),
    ),
    (
        "Kernel and systems",
        8,
        concat!(
            "kernel OR syscall OR eBPF OR \"memory management\" OR ",
            "scheduling OR \"systems programming\"",
        ),
    ),
    (
        "Distributed systems",
        9,
        concat!(
            "\"distributed systems\" OR consensus OR raft OR ",
            "replication OR consistency OR \"fault tolerance\"",
        ),
    ),
    (
        "Architecture",
        8,
        concat!(
            "architecture OR microservices OR \"system design\" OR ",
            "\"event driven\"",
        ),
    ),
    (
        "Data and databases",
        8,
        concat!(
            "database OR databases OR postgres OR PostgreSQL OR ",
            "Kafka OR CDC OR \"data pipeline\" OR transaction OR ",
            "indexing",
        ),
    ),
    (
        "Security and identity",
        8,
        concat!(
            "security OR vulnerability OR exploit OR ",
            "authentication OR authorization OR OAuth OR OpenID ",
            "OR OIDC OR cryptography",
        ),
    ),
    (
        "Platform and reliability",
        7,
        concat!(
            "SRE OR DevOps OR \"platform engineering\" OR ",
            "observability OR postmortem OR Terraform OR GitOps ",
            "OR \"incident analysis\"",
        ),
    ),
    (
        "AI and models",
        6,
        concat!(
            "transformer OR transformers OR LLM OR LLMs OR ",
            "inference OR \"language model\" OR \"machine learning\" ",
            "OR \"model training\"",
        ),
    ),
];
