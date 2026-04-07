#[allow(warnings)]
mod bindings;
mod html_to_md;

struct Component;

impl bindings::Guest for Component {
    fn html_to_markdown(html: String, selector: String) -> String {
        html_to_md::html_to_markdown(&html, None, &selector)
    }
}

bindings::export!(Component with_types_in bindings);
