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
    pub severities: Vec<String>,
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
                },
                "severities": {
                    "terms": {
                        "field": "severity.keyword",
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
    let severities = extract_bucket_keys(&body["aggregations"]["severities"]);

    Ok(AvailableFilters {
        environments,
        applications,
        severities,
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

pub struct LogResult {
    pub logs: Vec<LogEntry>,
    pub total: u64,
}

pub async fn fetch_logs(
    application: Option<&str>,
    profile: &str,
    severity: Option<&str>,
    time_range: &str,
    search: Option<&str>,
    search_exact: bool,
    size: i64,
    from: i64,
) -> Result<LogResult> {
    let client = create_client().await?;

    let mut must = vec![
        json!({"match": {"profiles": profile}}),
        json!({"range": {"@timestamp": {"gte": time_range}}}),
    ];
    if let Some(app) = application {
        must.push(json!({"match": {"application": app}}));
    }
    if let Some(sev) = severity {
        must.push(json!({"match": {"severity": sev}}));
    }
    if let Some(q) = search {
        if search_exact {
            must.push(json!({"match_phrase": {"message": q}}));
        } else {
            must.push(json!({"query_string": {"default_field": "message", "query": format!("*{}*", q)}}));
        }
    }

    let response = client
        .search(SearchParts::Index(&["logs-*"]))
        .body(json!({
            "query": { "bool": { "must": must } },
            "from": from,
            "size": size,
            "sort": [{"@timestamp": "desc"}],
            "track_total_hits": true
        }))
        .send()
        .await?;

    let body: Value = response.json().await?;

    let total = body["hits"]["total"]["value"].as_u64().unwrap_or(0);

    let hits = body["hits"]["hits"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No hits in response"))?;

    let logs: Vec<LogEntry> = hits
        .iter()
        .filter_map(|hit| serde_json::from_value(hit["_source"].clone()).ok())
        .collect();

    Ok(LogResult { logs, total })
}
