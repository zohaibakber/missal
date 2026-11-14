use roxmltree::Node;

pub const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const W_STRICT: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";
pub const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const R_STRICT: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";
pub const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
pub const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
pub const V: &str = "urn:schemas-microsoft-com:vml";
pub const M: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";

fn word_namespace(namespace: Option<&str>) -> bool {
    matches!(namespace, Some(W | W_STRICT))
}

pub fn is_w(node: Node, name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == name
        && word_namespace(node.tag_name().namespace())
}

pub fn is_ns(node: Node, namespace: &str, name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == name
        && node.tag_name().namespace() == Some(namespace)
}

pub fn w_name<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    (node.is_element() && word_namespace(node.tag_name().namespace()))
        .then(|| node.tag_name().name())
}

pub fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|child| is_w(*child, name))
}

pub fn children<'a, 'i>(node: Node<'a, 'i>, name: &str) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(move |child| is_w(*child, name))
}

pub fn attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.name() == name && word_namespace(attribute.namespace()))
        .map(|attribute| attribute.value())
}

pub fn r_attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| {
            attribute.name() == name && matches!(attribute.namespace(), Some(R | R_STRICT))
        })
        .map(|attribute| attribute.value())
}

pub fn plain_attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.name() == name && attribute.namespace().is_none())
        .map(|attribute| attribute.value())
}

pub fn val<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    attr(node, "val")
}

pub fn child_val<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    child(node, name).and_then(val)
}

pub fn on_off_value(value: Option<&str>) -> bool {
    !matches!(value, Some("0" | "false" | "off" | "none"))
}

pub fn on_off(node: Node) -> bool {
    on_off_value(val(node))
}

pub fn number(value: Option<&str>) -> Option<f64> {
    let value = value?.trim();
    let digits = value.strip_suffix('%').unwrap_or(value);
    let number: f64 = digits.parse().ok()?;
    number.is_finite().then_some(number)
}

pub fn twips(node: Node, name: &str) -> Option<f64> {
    let value = attr(node, name)?;
    measure(value)
}

pub fn measure(value: &str) -> Option<f64> {
    let value = value.trim();
    for (unit, factor) in [
        ("mm", 1440.0 / 25.4),
        ("cm", 1440.0 / 2.54),
        ("in", 1440.0),
        ("pt", 20.0),
        ("pc", 240.0),
        ("pi", 240.0),
    ] {
        if let Some(digits) = value.strip_suffix(unit) {
            return number(Some(digits)).map(|number| number * factor);
        }
    }
    number(Some(value))
}

pub fn text_of(node: Node) -> String {
    let mut text = String::new();
    for descendant in node.descendants() {
        if descendant.is_text() {
            text.push_str(descendant.text().unwrap_or_default());
        }
    }
    text
}

pub fn alternate_branch<'a, 'i>(node: Node<'a, 'i>) -> Option<Node<'a, 'i>> {
    const UNDERSTOOD: [&str; 8] = ["wps", "wpg", "wpc", "wp14", "w14", "w15", "a14", "v"];
    let mut fallback = None;
    for branch in node.children().filter(Node::is_element) {
        if is_ns(branch, MC, "Choice") {
            let requires = plain_attr(branch, "Requires").unwrap_or_default();
            if requires
                .split_whitespace()
                .all(|prefix| UNDERSTOOD.contains(&prefix))
            {
                return Some(branch);
            }
        } else if is_ns(branch, MC, "Fallback") {
            fallback = Some(branch);
        }
    }
    fallback
}
