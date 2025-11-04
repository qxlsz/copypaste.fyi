use crate::{BundleMetadata, PasteError, SharedPasteStore};

use super::models::PasteViewQuery;

pub async fn build_bundle_overview(
    store: SharedPasteStore,
    bundle: &BundleMetadata,
    query: &PasteViewQuery,
) -> Option<String> {
    if bundle.children.is_empty() {
        return None;
    }

    let mut items = String::new();
    for (idx, child) in bundle.children.iter().enumerate() {
        let label = child.label.as_deref().unwrap_or("");
        let label_display = if label.is_empty() {
            format!("Share {}", idx + 1)
        } else {
            label.to_string()
        };

        let status = match store.get_paste(&child.id).await {
            Ok(_) => ("available", "Available"),
            Err(PasteError::Expired(_)) => ("expired", "Expired"),
            Err(PasteError::NotFound(_)) => ("consumed", "Consumed"),
        };

        let url = build_child_url(&child.id, query);
        items.push_str(&format!(
            r#"        <li>
            <div class="bundle-link">
                <a href="{url}">{label}</a>
                <span class="status {class}">{status}</span>
                <code>{id}</code>
            </div>
        </li>
"#,
            url = html_escape::encode_safe(&url),
            label = html_escape::encode_safe(&label_display),
            class = status.0,
            status = status.1,
            id = html_escape::encode_safe(&child.id),
        ));
    }

    Some(format!(
        r#"<section class="bundle">
    <h2>Bundle shares</h2>
    <p>Each child paste burns after the first successful view.</p>
    <ul class="bundle-links">
{items}    </ul>
</section>
"#,
        items = items,
    ))
}

fn build_child_url(child_id: &str, query: &PasteViewQuery) -> String {
    if let Some(key) = query.key.as_ref() {
        format!("/{child_id}?key={}", urlencoding::encode(key))
    } else {
        format!("/{child_id}")
    }
}
