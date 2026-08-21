use crate::{BundleMetadata, PasteError, SharedPasteStore};

use super::models::PasteViewQuery;

pub async fn build_bundle_overview(
    store: SharedPasteStore,
    bundle: &BundleMetadata,
    query: &PasteViewQuery,
    is_blocked: impl Fn(&str) -> bool,
) -> Result<Option<String>, PasteError> {
    if bundle.children.is_empty() {
        return Ok(None);
    }

    let mut items = String::new();
    for (idx, child) in bundle.children.iter().enumerate() {
        // Quarantined children must not be linked, probed, or disclosed through
        // an otherwise accessible parent bundle.
        if is_blocked(&child.id) {
            continue;
        }
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
            Err(error @ PasteError::Persistence(_)) => return Err(error),
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

    if items.is_empty() {
        return Ok(None);
    }

    Ok(Some(format!(
        r#"<section class="bundle">
    <h2>Bundle shares</h2>
    <p>Child pastes request best-effort deletion after a view; concurrent readers may race.</p>
    <ul class="bundle-links">
{items}    </ul>
</section>
"#,
    )))
}

fn build_child_url(child_id: &str, query: &PasteViewQuery) -> String {
    if let Some(key) = query.key.as_ref() {
        format!("/p/{child_id}#key={}", urlencoding::encode(key))
    } else {
        format!("/p/{child_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_urls_keep_keys_out_of_request_targets() {
        let query = PasteViewQuery {
            key: Some("secret value".to_string()),
            ..Default::default()
        };
        let url = build_child_url("child-id", &query);

        assert_eq!(url, "/p/child-id#key=secret%20value");
        assert!(!url.contains("?key="));
    }
}
