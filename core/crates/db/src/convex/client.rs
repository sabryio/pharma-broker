//! Convex client wrapper
//!
//! Provides a type-safe wrapper around the Convex client for calling
//! Convex functions (queries and mutations).

use std::collections::BTreeMap;
use std::env;
use std::sync::Arc;

use convex::ConvexClient as BaseConvexClient;
use convex::Value;
use serde::de::DeserializeOwned;
use tokio::sync::RwLock;

use super::error::ConvexError;
use crate::Result;

/// Convex client wrapper with connection management
pub struct ConvexClient {
    client: Arc<RwLock<BaseConvexClient>>,
    url: String,
}

impl ConvexClient {
    /// Create a new Convex client from environment variables
    ///
    /// Reads `CONVEX_URL` (defaults to `http://127.0.0.1:3210`)
    pub async fn from_env() -> Result<Self> {
        let url = env::var("CONVEX_URL").unwrap_or_else(|_| "http://127.0.0.1:3210".to_string());
        Self::new(&url).await
    }

    /// Create a new Convex client with the given URL
    pub async fn new(url: &str) -> Result<Self> {
        let client = BaseConvexClient::new(url)
            .await
            .map_err(|e| ConvexError::Connection(e.to_string()))?;

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            url: url.to_string(),
        })
    }

    /// Get the Convex URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Execute a Convex query
    pub async fn query<T: DeserializeOwned>(
        &self,
        fn_name: &str,
        args: BTreeMap<String, Value>,
    ) -> Result<T> {
        let mut client = self.client.write().await;
        let result = client
            .query(fn_name, args)
            .await
            .map_err(|e| ConvexError::Query {
                function: fn_name.to_string(),
                message: e.to_string(),
            })?;

        // Convert Convex Value to the target type via JSON
        let json = convex_value_to_json(&result);
        serde_json::from_value(json).map_err(|e| {
            ConvexError::Deserialization {
                context: format!("query {}", fn_name),
                message: e.to_string(),
            }
            .into()
        })
    }

    /// Execute a Convex mutation
    pub async fn mutation<T: DeserializeOwned>(
        &self,
        fn_name: &str,
        args: BTreeMap<String, Value>,
    ) -> Result<T> {
        let mut client = self.client.write().await;
        let result = client
            .mutation(fn_name, args)
            .await
            .map_err(|e| ConvexError::Mutation {
                function: fn_name.to_string(),
                message: e.to_string(),
            })?;

        let json = convex_value_to_json(&result);
        serde_json::from_value(json).map_err(|e| {
            ConvexError::Deserialization {
                context: format!("mutation {}", fn_name),
                message: e.to_string(),
            }
            .into()
        })
    }

    /// Execute a Convex mutation that returns nothing
    pub async fn mutation_void(&self, fn_name: &str, args: BTreeMap<String, Value>) -> Result<()> {
        let mut client = self.client.write().await;
        client
            .mutation(fn_name, args)
            .await
            .map_err(|e| ConvexError::Mutation {
                function: fn_name.to_string(),
                message: e.to_string(),
            })?;
        Ok(())
    }

    /// Execute a Convex action
    pub async fn action<T: DeserializeOwned>(
        &self,
        fn_name: &str,
        args: BTreeMap<String, Value>,
    ) -> Result<T> {
        let mut client = self.client.write().await;
        let result = client
            .action(fn_name, args)
            .await
            .map_err(|e| ConvexError::Action {
                function: fn_name.to_string(),
                message: e.to_string(),
            })?;

        let json = convex_value_to_json(&result);
        serde_json::from_value(json).map_err(|e| {
            ConvexError::Deserialization {
                context: format!("action {}", fn_name),
                message: e.to_string(),
            }
            .into()
        })
    }
}

impl Clone for ConvexClient {
    fn clone(&self) -> Self {
        Self {
            client: Arc::clone(&self.client),
            url: self.url.clone(),
        }
    }
}

/// Convert Convex Value to serde_json::Value
fn convex_value_to_json(value: &convex::FunctionResult) -> serde_json::Value {
    // FunctionResult is essentially a Value, convert via debug repr or direct mapping
    // For simplicity, we'll use the Value's conversion if available
    match value {
        convex::FunctionResult::Value(v) => value_to_json(v),
        convex::FunctionResult::ErrorMessage(msg) => serde_json::json!({ "error": msg }),
        _ => serde_json::Value::Null,
    }
}

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Int64(i) => serde_json::json!(i),
        Value::Float64(f) => serde_json::json!(f),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Bytes(b) => serde_json::json!(b),
        Value::Array(arr) => serde_json::Value::Array(arr.iter().map(value_to_json).collect()),
        Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Helper to convert a value to Convex Value
pub fn to_value<T: serde::Serialize>(val: &T) -> Result<Value> {
    let json = serde_json::to_value(val).map_err(|e| ConvexError::Serialization {
        context: "to_value".to_string(),
        message: e.to_string(),
    })?;
    json_to_value(json)
}

fn json_to_value(json: serde_json::Value) -> Result<Value> {
    match json {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int64(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float64(f))
            } else {
                Ok(Value::Float64(0.0))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(s)),
        serde_json::Value::Array(arr) => {
            let values: Result<Vec<Value>> = arr.into_iter().map(json_to_value).collect();
            Ok(Value::Array(values?))
        }
        serde_json::Value::Object(obj) => {
            let mut map = BTreeMap::new();
            for (k, v) in obj {
                map.insert(k, json_to_value(v)?);
            }
            Ok(Value::Object(map))
        }
    }
}

/// Macro to create Convex function arguments
#[macro_export]
macro_rules! convex_args {
    () => {
        std::collections::BTreeMap::new()
    };
    ($($key:expr => $value:expr),+ $(,)?) => {{
        let mut map = std::collections::BTreeMap::new();
        $(
            map.insert($key.to_string(), $crate::convex::client::to_value(&$value)?);
        )+
        map
    }};
}
