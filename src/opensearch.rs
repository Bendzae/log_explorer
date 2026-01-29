use anyhow::Result;
use opensearch::http::transport::{SingleNodeConnectionPool, TransportBuilder};
use opensearch::{OpenSearch, SearchParts};
use serde::Deserialize;
use serde_json::{json, Value};
use url::Url;

const OPENSEARCH_URL: &str =
    "https://vpc-es-closelink-logs-ieziw6d36bxeyvrdgezcchssdi.eu-central-1.es.amazonaws.com";

#[derive(Debug, Clone, Deserialize)]
pub struct LogEntry {
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub application: String,
    #[serde(default)]
    pub logger: String,
    #[serde(default)]
    pub thread: String,
    #[serde(default)]
    pub profiles: String,
    #[serde(default)]
    pub method: String,
    #[serde(default, rename = "traceId")]
    pub trace_id: Option<String>,
}

#[derive(Debug)]
pub struct AvailableFilters {
    pub environments: Vec<String>,
    pub applications: Vec<String>,
}

async fn create_client() -> Result<OpenSearch> {
    let url = Url::parse(OPENSEARCH_URL)?;
    let conn_pool = SingleNodeConnectionPool::new(url);
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new("eu-central-1"))
        .load()
        .await;
    let transport = TransportBuilder::new(conn_pool)
        .auth(aws_config.clone().try_into()?)
        .build()?;
    Ok(OpenSearch::new(transport))
}

pub async fn fetch_available_filters() -> Result<AvailableFilters> {
    let client = create_client().await?;

    let response = client
        .search(SearchParts::Index(&["logs-*"]))
        .body(json!({
            "size": 0,
            "query": {
                "range": {"@timestamp": {"gte": "now-24h"}}
            },
            "aggs": {
                "applications": {
                    "terms": {
                        "field": "application.keyword",
                        "size": 100,
                        "order": {"_key": "asc"}
                    }
                },
                "profiles": {
                    "terms": {
                        "field": "profiles.keyword",
                        "size": 20,
                        "order": {"_key": "asc"}
                    }
                }
            }
        }))
        .send()
        .await?;

    let body: Value = response.json().await?;

    let environments = extract_bucket_keys(&body["aggregations"]["profiles"]);
    let applications = extract_bucket_keys(&body["aggregations"]["applications"]);

    Ok(AvailableFilters {
        environments,
        applications,
    })
}

fn extract_bucket_keys(agg: &Value) -> Vec<String> {
    agg["buckets"]
        .as_array()
        .map(|buckets| {
            buckets
                .iter()
                .filter_map(|b| b["key"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub async fn fetch_logs(application: &str, profile: &str, size: i64) -> Result<Vec<LogEntry>> {
    let client = create_client().await?;

    let response = client
        .search(SearchParts::Index(&["logs-*"]))
        .body(json!({
            "query": {
                "bool": {
                    "must": [
                        {"match": {"application": application}},
                        {"match": {"profiles": profile}},
                        {"range": {"@timestamp": {"gte": "now-24h"}}}
                    ]
                }
            },
            "size": size,
            "sort": [{"@timestamp": "desc"}]
        }))
        .send()
        .await?;

    let body: Value = response.json().await?;

    let hits = body["hits"]["hits"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No hits in response"))?;

    let logs: Vec<LogEntry> = hits
        .iter()
        .filter_map(|hit| serde_json::from_value(hit["_source"].clone()).ok())
        .collect();

    Ok(logs)
}
