use std::collections::HashSet;

use serde_json::{json, Value};

use crate::{plugin::PluginRegistry, store::Store, Result};

#[derive(Clone)]
pub(super) struct ListedModel {
    pub public_id: String,
    pub aliases: Vec<String>,
    pub internal_id: String,
    pub display_name: String,
}

pub(super) async fn list(store: &Store, plugins: &PluginRegistry) -> Result<Vec<ListedModel>> {
    let mut models = Vec::new();
    let mut ids = HashSet::new();
    let mut configured = store.models().await?;
    configured.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.model_hash.cmp(&right.model_hash))
    });
    for model in configured {
        let group = model
            .group_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                url::Url::parse(&model.base_url)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_owned))
                    .unwrap_or_else(|| model.base_url.clone())
            });
        let public_id = format!("{group}/{}", model.model_id);
        if ids.insert(public_id.clone()) {
            models.push(ListedModel {
                public_id,
                aliases: Vec::new(),
                internal_id: model.model_hash,
                display_name: model.display_name,
            });
        }
    }
    for model in plugins.configured_models().await {
        // 外部工具（尤其只显示模型 ID 的选择器）读不下 `plugin:<plugin>:<provider>/<model>`
        // 这么长的 ID，因此对外优先暴露短名 `<provider>/<model>`；
        // 旧的长 ID 仍作为别名参与解析，已写入的配置不会失效。
        let legacy = format!(
            "plugin:{}:{}/{}",
            model.plugin_id, model.provider_id, model.model_id
        );
        let short = format!("{}/{}", model.provider_id, model.model_id);
        let public_id = if ids.contains(&short) {
            legacy.clone()
        } else {
            short
        };
        if !ids.insert(public_id.clone()) {
            continue;
        }
        ids.insert(legacy.clone());
        let aliases = if public_id == legacy {
            Vec::new()
        } else {
            vec![legacy]
        };
        models.push(ListedModel {
            public_id,
            aliases,
            internal_id: model.id,
            display_name: model.display_name,
        });
    }
    Ok(models)
}

pub(super) fn response(models: &[ListedModel]) -> Value {
    json!({"object":"list","data":models.iter().map(|model| json!({
        "id":model.public_id,"object":"model","created":0,"owned_by":"cursor-byok",
        "name":model.display_name
    })).collect::<Vec<_>>()})
}
