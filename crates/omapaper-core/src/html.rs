//! The HTML node model behind Paper-style tools.
//!
//! Frames are real HTML documents, so OmaPaper reads them with html5ever and
//! addresses elements two ways: by `data-op-id`, or by a CSS selector (the
//! supported subset is `tag`, `#id`, `.class`, `[attr]`, `[attr="value"]` and
//! descendant combinators — no pseudo-classes).
//!
//! Edits re-serialise the document. That normalises formatting (tags closed,
//! `<html>`/`<head>`/`<body>` made explicit) and is why the HTML panel exists
//! for byte-exact work.

use html5ever::serialize::{SerializeOpts, TraversalScope};
use html5ever::tendril::TendrilSink;
use html5ever::{namespace_url, ns, parse_document, serialize, Attribute, LocalName, QualName};
use markup5ever_rcdom::{Handle, Node, NodeData, RcDom, SerializableHandle};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug, thiserror::Error)]
pub enum HtmlError {
    #[error("selector: {0}")]
    Selector(String),
    #[error("no element matches `{0}`")]
    NotFound(String),
    #[error("`{0}` matches {1} elements; pass `index`")]
    Ambiguous(String, usize),
    #[error("html: {0}")]
    Html(String),
}

pub type Result<T> = std::result::Result<T, HtmlError>;

/// A matched element plus what the tools report about it.
#[derive(Debug, Clone)]
pub struct Matched {
    pub node: Handle,
    pub tag: String,
    pub path: String,
}

/// Parses a document.
pub fn parse(html: &str) -> RcDom {
    parse_document(RcDom::default(), Default::default()).one(html)
}

/// Serialises the whole document (doctype and all).
pub fn serialize_document(dom: &RcDom) -> String {
    let mut bytes = Vec::new();
    let handle: SerializableHandle = dom.document.clone().into();
    serialize(&mut bytes, &handle, SerializeOpts::default()).ok();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Serialises one node, tag and children included.
pub fn serialize_node(node: &Handle) -> String {
    let mut bytes = Vec::new();
    let handle: SerializableHandle = node.clone().into();
    let opts = SerializeOpts {
        traversal_scope: TraversalScope::IncludeNode,
        ..SerializeOpts::default()
    };
    serialize(&mut bytes, &handle, opts).ok();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn element_name(node: &Handle) -> Option<String> {
    match &node.data {
        NodeData::Element { name, .. } => Some(name.local.to_string()),
        _ => None,
    }
}

fn attrs(node: &Handle) -> Vec<Attribute> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs.borrow().clone(),
        _ => Vec::new(),
    }
}

fn attr_value(node: &Handle, name: &str) -> Option<String> {
    attrs(node)
        .into_iter()
        .find(|a| a.name.local.as_ref() == name)
        .map(|a| a.value.to_string())
}

fn set_attr_value(node: &Handle, name: &str, value: &str) {
    if let NodeData::Element { attrs, .. } = &node.data {
        let mut list = attrs.borrow_mut();
        if let Some(slot) = list.iter_mut().find(|a| a.name.local.as_ref() == name) {
            slot.value = value.into();
        } else {
            list.push(Attribute {
                name: QualName::new(None, ns!(), LocalName::from(name)),
                value: value.into(),
            });
        }
    }
}

fn remove_attr(node: &Handle, name: &str) {
    if let NodeData::Element { attrs, .. } = &node.data {
        attrs.borrow_mut().retain(|a| a.name.local.as_ref() != name);
    }
}

/// The `<body>` element, or the document when a fragment has none.
pub fn body(dom: &RcDom) -> Handle {
    let mut found = None;
    walk(dom.document.clone(), &mut |node, _| {
        if found.is_none() && element_name(node).as_deref() == Some("body") {
            found = Some(node.clone());
        }
    });
    found.unwrap_or_else(|| dom.document.clone())
}

/// Every element under `<body>` with a readable path.
pub fn elements(html: &str) -> Vec<Matched> {
    let dom = parse(html);
    let body = body(&dom);
    let mut out = Vec::new();
    walk(body.clone(), &mut |node, path| {
        if let Some(tag) = element_name(node) {
            out.push(Matched {
                node: node.clone(),
                tag,
                path: path.to_string(),
            });
        }
    });
    out
}

/// Depth-first walk, building a path like `body > div:nth-child(2) > h1`.
fn walk(node: Handle, f: &mut impl FnMut(&Handle, &str)) {
    fn inner(node: Handle, path: String, f: &mut impl FnMut(&Handle, &str)) {
        f(&node, &path);
        let children = node.children.borrow().clone();
        let mut index = 0usize;
        for child in children {
            let tag = match element_name(&child) {
                Some(tag) => tag,
                None => continue,
            };
            index += 1;
            let next = if path.is_empty() {
                format!("{tag}:nth-child({index})")
            } else {
                format!("{path} > {tag}:nth-child({index})")
            };
            inner(child, next, f);
        }
    }
    inner(node, String::new(), f);
}

// ---- selectors ------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(String, Option<String>)>,
}

fn parse_compound(text: &str) -> Result<Compound> {
    if text.is_empty() {
        return Err(HtmlError::Selector("empty selector part".into()));
    }
    if text.contains(':') {
        return Err(HtmlError::Selector(format!(
            "pseudo-classes are not supported (`{text}`)"
        )));
    }
    if text.contains('>') || text.contains('+') || text.contains('~') {
        return Err(HtmlError::Selector(format!(
            "combinators other than a descendant space are not supported (`{text}`)"
        )));
    }
    let mut compound = Compound::default();
    let mut rest = text;
    // Leading tag.
    let tag_end = rest.find(['#', '.', '[']).unwrap_or(rest.len());
    if tag_end > 0 {
        compound.tag = Some(rest[..tag_end].to_string());
        rest = &rest[tag_end..];
    }
    while !rest.is_empty() {
        let marker = rest.chars().next().unwrap();
        match marker {
            '#' => {
                let end = rest[1..]
                    .find(['#', '.', '['])
                    .map(|i| i + 1)
                    .unwrap_or(rest.len());
                compound.id = Some(rest[1..end].to_string());
                rest = &rest[end..];
            }
            '.' => {
                let end = rest[1..]
                    .find(['#', '.', '['])
                    .map(|i| i + 1)
                    .unwrap_or(rest.len());
                compound.classes.push(rest[1..end].to_string());
                rest = &rest[end..];
            }
            '[' => {
                let close = rest
                    .find(']')
                    .ok_or_else(|| HtmlError::Selector(format!("unclosed `[` in `{text}`")))?;
                let inner = &rest[1..close];
                let (name, value) = match inner.split_once('=') {
                    Some((name, value)) => (
                        name.trim().to_string(),
                        Some(value.trim().trim_matches(['"', '\'']).to_string()),
                    ),
                    None => (inner.trim().to_string(), None),
                };
                compound.attrs.push((name, value));
                rest = &rest[close + 1..];
            }
            _ => {
                return Err(HtmlError::Selector(format!(
                    "unexpected `{marker}` in `{text}`"
                )))
            }
        }
    }
    Ok(compound)
}

fn parse_selector(selector: &str) -> Result<Vec<Vec<Compound>>> {
    let mut groups = Vec::new();
    for group in selector.split(',') {
        let group = group.trim();
        if group.is_empty() {
            return Err(HtmlError::Selector(format!(
                "empty selector in `{selector}`"
            )));
        }
        let compounds = group
            .split_whitespace()
            .map(parse_compound)
            .collect::<Result<Vec<_>>>()?;
        groups.push(compounds);
    }
    Ok(groups)
}

fn matches_compound(node: &Handle, compound: &Compound) -> bool {
    let Some(tag) = element_name(node) else {
        return false;
    };
    if let Some(want) = &compound.tag {
        if !tag.eq_ignore_ascii_case(want) {
            return false;
        }
    }
    if let Some(want) = &compound.id {
        if attr_value(node, "id").as_deref() != Some(want.as_str()) {
            return false;
        }
    }
    if !compound.classes.is_empty() {
        let classes = attr_value(node, "class").unwrap_or_default();
        let classes: Vec<&str> = classes.split_whitespace().collect();
        if !compound
            .classes
            .iter()
            .all(|c| classes.contains(&c.as_str()))
        {
            return false;
        }
    }
    for (name, value) in &compound.attrs {
        match (attr_value(node, name), value) {
            (None, _) => return false,
            (Some(_), None) => {}
            (Some(actual), Some(want)) if &actual == want => {}
            _ => return false,
        }
    }
    true
}

fn matches_group(node: &Handle, group: &[Compound]) -> bool {
    let Some(last) = group.last() else {
        return false;
    };
    if !matches_compound(node, last) {
        return false;
    }
    let mut current = node.clone();
    for compound in group.iter().rev().skip(1) {
        let mut found = false;
        loop {
            let weak = current.parent.take();
            let Some(weak) = weak else { break };
            let parent = weak.upgrade();
            current.parent.set(Some(weak));
            let Some(parent) = parent else { break };
            if matches_compound(&parent, compound) {
                current = parent;
                found = true;
                break;
            }
            if element_name(&parent).is_none() {
                break;
            }
            current = parent;
        }
        if !found {
            return false;
        }
    }
    true
}

/// Every element matching `selector`, in document order.
pub fn find(html: &str, selector: &str) -> Result<Vec<Matched>> {
    let groups = parse_selector(selector)?;
    let dom = parse(html);
    let body = body(&dom);
    let mut out = Vec::new();
    walk(body.clone(), &mut |node, path| {
        if element_name(node).is_none() {
            return;
        }
        if groups.iter().any(|group| matches_group(node, group)) {
            out.push(Matched {
                node: node.clone(),
                tag: element_name(node).unwrap_or_default(),
                path: path.to_string(),
            });
        }
    });
    Ok(out)
}

/// The selector for a node id, quoted safely.
pub fn selector_for_id(id: &str) -> String {
    format!("[data-op-id=\"{}\"]", id.replace('"', "\\\""))
}

/// Resolves a `node_id` or `selector` pair into matches.
pub fn resolve(html: &str, node_id: Option<&str>, selector: Option<&str>) -> Result<Vec<Matched>> {
    match (node_id, selector) {
        (Some(id), _) => {
            let matches = find(html, &selector_for_id(id))?;
            if matches.is_empty() {
                Err(HtmlError::NotFound(format!("data-op-id=\"{id}\"")))
            } else {
                Ok(matches)
            }
        }
        (None, Some(selector)) => {
            let matches = find(html, selector)?;
            if matches.is_empty() {
                Err(HtmlError::NotFound(selector.to_string()))
            } else {
                Ok(matches)
            }
        }
        (None, None) => Err(HtmlError::Selector("pass `nodeId` or `selector`".into())),
    }
}

fn pick(matches: Vec<Matched>, selector: &str, index: Option<usize>) -> Result<Vec<Matched>> {
    match index {
        Some(index) => matches
            .into_iter()
            .nth(index)
            .map(|m| vec![m])
            .ok_or_else(|| HtmlError::NotFound(format!("{selector}[{index}]"))),
        None if matches.len() > 1 => Err(HtmlError::Ambiguous(selector.to_string(), matches.len())),
        None => Ok(matches),
    }
}

// ---- styles ---------------------------------------------------------------

/// `fontSize` -> `font-size`; already-kebab values pass through.
pub fn kebab(key: &str) -> String {
    let mut out = String::new();
    for (index, ch) in key.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// `font-size` -> `fontSize`.
pub fn camel(key: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for ch in key.chars() {
        if ch == '-' {
            upper = true;
        } else if upper {
            out.push(ch.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Parses an inline `style` attribute.
pub fn parse_style(value: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for declaration in value.split(';') {
        let Some((key, value)) = declaration.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if !key.is_empty() && !value.is_empty() {
            map.insert(kebab(key), value.to_string());
        }
    }
    map
}

fn style_string(map: &BTreeMap<String, String>) -> String {
    map.iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Merges `styles` into every match's inline style. Empty values remove a
/// property. Returns the new document and how many elements changed.
pub fn update_styles(
    html: &str,
    selector: &str,
    styles: &BTreeMap<String, String>,
    index: Option<usize>,
    all: bool,
) -> Result<(String, usize)> {
    let matches = find(html, selector)?;
    let count = if all {
        matches.len()
    } else {
        pick(matches.clone(), selector, index)?.len()
    };
    let updated = replace_document(html, selector, index, all, |node| {
        let mut map = parse_style(&attr_value(node, "style").unwrap_or_default());
        for (key, value) in styles {
            let key = kebab(key);
            if value.trim().is_empty() {
                map.remove(&key);
            } else {
                map.insert(key, value.trim().to_string());
            }
        }
        if map.is_empty() {
            remove_attr(node, "style");
        } else {
            set_attr_value(node, "style", &style_string(&map));
        }
    })?;
    Ok((updated, count))
}

/// Applies `edit` to the single matched node inside one parsed document and
/// returns the serialised document.
fn replace_document(
    html: &str,
    selector: &str,
    index: Option<usize>,
    all: bool,
    edit: impl Fn(&Handle),
) -> Result<String> {
    let dom = parse(html);
    let body = body(&dom);
    let groups = parse_selector(selector)?;
    let mut matches = Vec::new();
    walk(body, &mut |node, _| {
        if element_name(node).is_none() {
            return;
        }
        if groups.iter().any(|group| matches_group(node, group)) {
            matches.push(node.clone());
        }
    });
    if matches.is_empty() {
        return Err(HtmlError::NotFound(selector.to_string()));
    }
    let targets: Vec<Handle> = match index {
        Some(index) => matches
            .into_iter()
            .nth(index)
            .map(|n| vec![n])
            .ok_or_else(|| HtmlError::NotFound(format!("{selector}[{index}]")))?,
        None if !all && matches.len() > 1 => {
            return Err(HtmlError::Ambiguous(selector.to_string(), matches.len()))
        }
        None => matches,
    };
    for node in &targets {
        edit(node);
    }
    Ok(serialize_document(&dom))
}

/// Replaces an element's text with `text`, keeping the element itself.
pub fn set_text(html: &str, selector: &str, text: &str, index: Option<usize>, all: bool) -> Result<String> {
    replace_document(html, selector, index, all, |node| {
        node.children.borrow_mut().clear();
        let child = Node::new(NodeData::Text {
            contents: RefCell::new(text.into()),
        });
        node.children.borrow_mut().push(child);
    })
}

/// Renames a layer (`data-name`), leaving the HTML structure alone.
pub fn rename(html: &str, selector: &str, name: &str, index: Option<usize>, all: bool) -> Result<String> {
    replace_document(html, selector, index, all, |node| {
        set_attr_value(node, "data-name", name);
    })
}

/// Deletes the matched elements.
pub fn delete(html: &str, selector: &str, index: Option<usize>, _all: bool) -> Result<String> {
    let dom = parse(html);
    let body = body(&dom);
    let groups = parse_selector(selector)?;
    let mut matches = Vec::new();
    walk(body, &mut |node, _| {
        if element_name(node).is_none() {
            return;
        }
        if groups.iter().any(|group| matches_group(node, group)) {
            matches.push(node.clone());
        }
    });
    if matches.is_empty() {
        return Err(HtmlError::NotFound(selector.to_string()));
    }
    let targets: Vec<Handle> = match index {
        Some(index) => matches
            .into_iter()
            .nth(index)
            .map(|n| vec![n])
            .ok_or_else(|| HtmlError::NotFound(format!("{selector}[{index}]")))?,
        None => matches,
    };
    for node in &targets {
        detach(node);
    }
    Ok(serialize_document(&dom))
}

fn detach(node: &Handle) {
    let weak = node.parent.take();
    if let Some(weak) = &weak {
        if let Some(parent) = weak.upgrade() {
            parent
                .children
                .borrow_mut()
                .retain(|child| !Rc::ptr_eq(child, node));
        }
    }
    node.parent.set(weak);
}

// ---- read-side tools ------------------------------------------------------

/// A compact tree of the document, for `get_tree_summary`.
pub fn tree_json(html: &str, max_depth: usize) -> serde_json::Value {
    fn build(node: &Handle, path: &str, depth: usize, max_depth: usize) -> serde_json::Value {
        let tag = element_name(node).unwrap_or_else(|| "#document".into());
        let mut map = serde_json::Map::new();
        map.insert("tag".into(), tag.clone().into());
        if !path.is_empty() {
            map.insert("path".into(), path.into());
        }
        if let Some(id) = attr_value(node, "data-op-id") {
            map.insert("id".into(), id.into());
        }
        if let Some(name) = attr_value(node, "data-name") {
            map.insert("name".into(), name.into());
        }
        if let Some(class) = attr_value(node, "class") {
            map.insert("class".into(), class.into());
        }
        if let Some(text) = direct_text(node) {
            map.insert("text".into(), text.into());
        }
        let children = node.children.borrow().clone();
        let mut kids = Vec::new();
        if depth < max_depth {
            let mut index = 0usize;
            for child in children {
                let Some(child_tag) = element_name(&child) else {
                    continue;
                };
                index += 1;
                let child_path = if path.is_empty() {
                    format!("{child_tag}:nth-child({index})")
                } else {
                    format!("{path} > {child_tag}:nth-child({index})")
                };
                kids.push(build(&child, &child_path, depth + 1, max_depth));
            }
        }
        if !kids.is_empty() {
            map.insert("children".into(), serde_json::Value::Array(kids));
        }
        serde_json::Value::Object(map)
    }

    let dom = parse(html);
    let body = body(&dom);
    build(&body, "", 0, max_depth)
}

fn direct_text(node: &Handle) -> Option<String> {
    let mut text = String::new();
    for child in node.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            text.push_str(&contents.borrow().to_string());
        }
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        None
    } else if text.chars().count() > 90 {
        Some(text.chars().take(90).collect::<String>() + "…")
    } else {
        Some(text)
    }
}

/// Text of every element, whitespace-collapsed (used by search).
pub fn text_content(html: &str) -> String {
    let dom = parse(html);
    fn collect(node: &Handle, out: &mut String) {
        for child in node.children.borrow().iter() {
            match &child.data {
                NodeData::Text { contents } => {
                    out.push_str(&contents.borrow().to_string());
                    out.push(' ');
                }
                NodeData::Element { name, .. } => {
                    if name.local.as_ref() == "script" || name.local.as_ref() == "style" {
                        continue;
                    }
                    collect(child, out);
                }
                _ => {}
            }
        }
    }
    let body = body(&dom);
    let mut out = String::new();
    collect(&body, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One compact record per match, for `find_nodes`.
pub fn find_json(html: &str, selector: &str) -> Result<Vec<serde_json::Value>> {
    Ok(find(html, selector)?
        .iter()
        .map(|matched| node_value(&matched.node, &matched.tag, &matched.path))
        .collect())
}

/// The direct element children of the first match, for `get_children`.
pub fn children_json(html: &str, selector: &str) -> Result<Vec<serde_json::Value>> {
    let matches = find(html, selector)?;
    let Some(parent) = matches.first() else {
        return Err(HtmlError::NotFound(selector.to_string()));
    };
    let mut out = Vec::new();
    for child in parent.node.children.borrow().iter() {
        if let Some(tag) = element_name(child) {
            let path = format!("{selector} > {tag}");
            out.push(node_value(child, &tag, &path));
        }
    }
    Ok(out)
}

fn node_value(node: &Handle, tag: &str, path: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("tag".into(), tag.into());
    map.insert("path".into(), path.into());
    if let Some(id) = attr_value(node, "data-op-id") {
        map.insert("id".into(), id.into());
    }
    if let Some(name) = attr_value(node, "data-name") {
        map.insert("name".into(), name.into());
    }
    if let Some(class) = attr_value(node, "class") {
        map.insert("class".into(), class.into());
    }
    if let Some(text) = direct_text(node) {
        map.insert("text".into(), text.into());
    }
    map.insert("children".into(), node.children.borrow().len().into());
    serde_json::Value::Object(map)
}

/// The node's own attributes as a map.
pub fn node_json(html: &str, selector: &str) -> Result<serde_json::Value> {
    let matches = find(html, selector)?;
    let Some(matched) = matches.first() else {
        return Err(HtmlError::NotFound(selector.to_string()));
    };
    let mut map = serde_json::Map::new();
    map.insert("tag".into(), matched.tag.clone().into());
    map.insert("path".into(), matched.path.clone().into());
    if let Some(id) = attr_value(&matched.node, "data-op-id") {
        map.insert("id".into(), id.into());
    }
    let attributes: serde_json::Map<String, serde_json::Value> = attrs(&matched.node)
        .into_iter()
        .map(|a| (a.name.local.to_string(), a.value.to_string().into()))
        .collect();
    map.insert("attributes".into(), serde_json::Value::Object(attributes));
    let styles: serde_json::Map<String, serde_json::Value> =
        parse_style(&attr_value(&matched.node, "style").unwrap_or_default())
            .into_iter()
            .map(|(k, v)| (camel(&k), v.into()))
            .collect();
    map.insert("styles".into(), serde_json::Value::Object(styles));
    if let Some(text) = direct_text(&matched.node) {
        map.insert("text".into(), text.into());
    }
    map.insert(
        "children".into(),
        matched.node.children.borrow().len().into(),
    );
    Ok(serde_json::Value::Object(map))
}

/// Inline styles plus matching `<style>` rules (cascade order: sheet, inline).
pub fn computed_styles(html: &str, selector: &str) -> Result<BTreeMap<String, String>> {
    let matches = find(html, selector)?;
    let Some(matched) = matches.first() else {
        return Err(HtmlError::NotFound(selector.to_string()));
    };
    let dom = parse(html);
    let mut computed = BTreeMap::new();
    // Collect every <style> rule, in document order.
    let mut rules: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    walk(dom.document.clone(), &mut |node, _| {
        if element_name(node).as_deref() != Some("style") {
            return;
        }
        let mut css = String::new();
        for child in node.children.borrow().iter() {
            if let NodeData::Text { contents } = &child.data {
                css.push_str(&contents.borrow().to_string());
            }
        }
        rules.extend(parse_rules(&css));
    });
    for (rule_selector, declarations) in rules {
        let Ok(groups) = parse_selector(&rule_selector) else {
            continue;
        };
        if groups
            .iter()
            .any(|group| matches_group(&matched.node, group))
        {
            computed.extend(declarations);
        }
    }
    computed.extend(parse_style(
        &attr_value(&matched.node, "style").unwrap_or_default(),
    ));
    Ok(computed.into_iter().map(|(k, v)| (camel(&k), v)).collect())
}

fn parse_rules(css: &str) -> Vec<(String, BTreeMap<String, String>)> {
    let mut rules = Vec::new();
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let selector_text = rest[..open].trim();
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let body = &rest[open + 1..open + close];
        for selector in selector_text.split(',') {
            let selector = selector.trim();
            if !selector.is_empty() && !selector.starts_with('@') {
                rules.push((selector.to_string(), parse_style(body)));
            }
        }
        rest = &rest[open + close + 1..];
    }
    rules
}

/// JSX for the document body, for handing a frame to a codebase.
pub fn to_jsx(html: &str) -> String {
    let dom = parse(html);
    let body = body(&dom);
    let children = body.children.borrow().clone();
    let mut parts = Vec::new();
    for child in children {
        if let Some(jsx) = jsx_node(&child, 0) {
            parts.push(jsx);
        }
    }
    parts.join("\n")
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

fn jsx_node(node: &Handle, depth: usize) -> Option<String> {
    let pad = "  ".repeat(depth);
    match &node.data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                return None;
            }
            let escaped = text.replace('{', "&#123;").replace('}', "&#125;");
            Some(format!("{pad}{escaped}"))
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.to_string();
            if matches!(tag.as_str(), "script" | "style" | "meta" | "title" | "link") {
                return None;
            }
            let mut attributes = Vec::new();
            for attribute in attrs(node) {
                let key = attribute.name.local.to_string();
                let value = attribute.value.to_string();
                match key.as_str() {
                    "class" => attributes.push(format!("className=\"{}\"", escape_attr(&value))),
                    "for" => attributes.push(format!("htmlFor=\"{}\"", escape_attr(&value))),
                    "style" => {
                        let map = parse_style(&value);
                        if !map.is_empty() {
                            let entries = map
                                .iter()
                                .map(|(k, v)| format!("\"{}\": \"{}\"", camel(k), escape_attr(v)))
                                .collect::<Vec<_>>()
                                .join(", ");
                            attributes.push(format!("style={{{{ {entries} }}}}"));
                        }
                    }
                    _ => attributes.push(format!("{key}=\"{}\"", escape_attr(&value))),
                }
            }
            let attrs = if attributes.is_empty() {
                String::new()
            } else {
                format!(" {}", attributes.join(" "))
            };
            if VOID.contains(&tag.as_str()) {
                return Some(format!("{pad}<{tag}{attrs} />"));
            }
            let mut children = Vec::new();
            for child in node.children.borrow().iter() {
                if let Some(jsx) = jsx_node(child, depth + 1) {
                    children.push(jsx);
                }
            }
            if children.is_empty() {
                Some(format!("{pad}<{tag}{attrs}></{tag}>"))
            } else {
                Some(format!(
                    "{pad}<{tag}{attrs}>\n{}\n{pad}</{tag}>",
                    children.join("\n")
                ))
            }
        }
        _ => None,
    }
}

fn escape_attr(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

/// Counts elements in a document.
pub fn element_count(html: &str) -> usize {
    fn count(node: &Handle) -> usize {
        node.children
            .borrow()
            .iter()
            .filter(|child| element_name(child).is_some())
            .map(|child| 1 + count(child))
            .sum()
    }
    count(&body(&parse(html)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"<!doctype html><html><head><title>T</title><style>.card { color: #123456; padding: 4px; }</style></head>
<body><main data-op-id="hero" class="card hero"><h1 class="title">Hello</h1><p>World <b>now</b></p><img src="a.png" alt="x"></main></body></html>"#;

    #[test]
    fn finds_by_tag_class_id_and_attr() {
        assert_eq!(find(DOC, "h1").unwrap().len(), 1);
        assert_eq!(find(DOC, ".card").unwrap().len(), 1);
        assert_eq!(
            find(DOC, "#hero").unwrap().len(),
            0,
            "id= means the id attribute"
        );
        assert_eq!(find(DOC, "[data-op-id]").unwrap().len(), 1);
        assert_eq!(find(DOC, "main h1").unwrap().len(), 1);
        assert!(find(DOC, "main > h1").is_err());
        assert_eq!(find(DOC, "section").unwrap().len(), 0);
    }

    #[test]
    fn updates_inline_styles() {
        let mut styles = BTreeMap::new();
        styles.insert("backgroundColor".into(), "#fff".into());
        styles.insert("padding".into(), "16px".into());
        let (html, count) = update_styles(DOC, "h1", &styles, None, false).unwrap();
        assert_eq!(count, 1);
        assert!(html.contains("background-color: #fff"));
        assert!(html.contains("padding: 16px"));
        // Removing with an empty value.
        let mut remove = BTreeMap::new();
        remove.insert("padding".into(), String::new());
        let (html, _) = update_styles(&html, "h1", &remove, None, false).unwrap();
        assert!(!html.contains("padding: 16px"));
    }

    #[test]
    fn sets_text_and_renames() {
        let html = set_text(DOC, "h1", "Bonjour", None, false).unwrap();
        assert!(html.contains(">Bonjour<"));
        let html = rename(&html, "main", "Hero", None, false).unwrap();
        assert!(html.contains("data-name=\"Hero\""));
    }

    #[test]
    fn deletes_nodes() {
        let html = delete(DOC, "p", None, false).unwrap();
        assert!(!html.contains("<p>"));
        assert!(html.contains("<h1"));
    }

    #[test]
    fn computed_styles_cascade_the_sheet_then_inline() {
        let styles = computed_styles(DOC, ".card").unwrap();
        assert_eq!(styles.get("color").map(String::as_str), Some("#123456"));
        assert_eq!(styles.get("padding").map(String::as_str), Some("4px"));
        let html = update_styles(
            DOC,
            ".card",
            &BTreeMap::from([("color".to_string(), "#000".to_string())]),
            None,
            false,
        )
        .unwrap()
        .0;
        let styles = computed_styles(&html, ".card").unwrap();
        assert_eq!(styles.get("color").map(String::as_str), Some("#000"));
    }

    #[test]
    fn jsx_is_valid_looking() {
        let jsx = to_jsx(DOC);
        assert!(jsx.contains("<main data-op-id=\"hero\" className=\"card hero\">"));
        assert!(jsx.contains("<h1 className=\"title\">"));
        assert!(jsx.contains("<img src=\"a.png\" alt=\"x\" />"));
        assert!(!jsx.contains("<style"));
    }

    #[test]
    fn tree_reports_ids_names_and_depth() {
        let tree = tree_json(DOC, 10);
        let text = tree.to_string();
        assert!(text.contains("\"tag\":\"body\""));
        assert!(text.contains("\"id\":\"hero\""));
        assert!(text.contains("\"text\":\"World\""));
        let shallow = tree_json(DOC, 0);
        assert!(!shallow.to_string().contains("children"));
    }

    #[test]
    fn ambiguous_selectors_need_an_index() {
        let err = update_styles(
            DOC,
            "h1",
            &BTreeMap::from([("color".to_string(), "red".to_string())]),
            Some(3),
            false,
        )
        .unwrap_err();
        assert!(matches!(err, HtmlError::NotFound(_)));
    }

    #[test]
    fn text_content_skips_style_and_script() {
        let text = text_content(DOC);
        assert!(text.contains("Hello"));
        assert!(!text.contains("color"));
    }
}
