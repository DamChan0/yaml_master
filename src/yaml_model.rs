use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use anyhow::{anyhow, Result};
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodePath(pub Vec<PathSegment>);

impl NodePath {
    pub fn dot_path(&self) -> String {
        let mut out = String::new();
        for (idx, seg) in self.0.iter().enumerate() {
            if idx > 0 {
                out.push('.');
            }
            match seg {
                PathSegment::Key(key) => out.push_str(key),
                PathSegment::Index(index) => out.push_str(&index.to_string()),
            }
        }
        out
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    pub fn child_key(&self, key: &str) -> Self {
        let mut next = self.0.clone();
        next.push(PathSegment::Key(key.to_string()));
        Self(next)
    }

    pub fn child_index(&self, index: usize) -> Self {
        let mut next = self.0.clone();
        next.push(PathSegment::Index(index));
        Self(next)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeType {
    Map,
    Seq,
    String,
    Number,
    Bool,
    Null,
    Unknown,
}

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            NodeType::Map => "map",
            NodeType::Seq => "seq",
            NodeType::String => "string",
            NodeType::Number => "number",
            NodeType::Bool => "bool",
            NodeType::Null => "null",
            NodeType::Unknown => "unknown",
        };
        write!(f, "{label}")
    }
}

#[derive(Clone, Debug)]
pub struct VisibleRow {
    pub path: NodePath,
    pub depth: usize,
    pub display_key: String,
    pub display_value_preview: String,
    pub node_type: NodeType,
    pub is_container: bool,
}

#[derive(Clone, Debug)]
pub struct TreeNode {
    pub path: NodePath,
    pub key: String,
    pub node_type: NodeType,
    pub value_preview: String,
    pub children: Vec<TreeNode>,
}

pub struct YamlModel {
    doc: Yaml,
    path: String,
}

impl YamlModel {
    pub fn load(path: &Path) -> Result<Self> {
        let (model, err, _) = Self::load_with_error(path)?;
        if let Some(e) = err {
            return Err(anyhow!("{}", e));
        }
        Ok(model)
    }

    /// Load YAML; on parse error returns empty doc, error message, and raw content so the file can be edited.
    pub fn load_with_error(path: &Path) -> Result<(Self, Option<String>, Option<String>)> {
        let input = std::fs::read_to_string(path)?;
        let path_str = path.display().to_string();
        match YamlLoader::load_from_str(&input) {
            Ok(docs) => {
                let doc = docs.into_iter().next().unwrap_or(Yaml::Null);
                Ok((
                    Self {
                        doc,
                        path: path_str,
                    },
                    None,
                    None,
                ))
            }
            Err(e) => {
                let err_msg = e.to_string();
                Ok((
                    Self {
                        doc: Yaml::Null,
                        path: path_str.clone(),
                    },
                    Some(err_msg),
                    Some(input),
                ))
            }
        }
    }

    /// Empty model for file picker state (no file loaded yet).
    pub fn empty() -> Self {
        Self {
            doc: Yaml::Null,
            path: String::new(),
        }
    }

    /// Path of the currently loaded file (for "open another file").
    pub fn file_path(&self) -> &str {
        &self.path
    }

    pub fn save(&self) -> Result<()> {
        let mut out = String::new();
        let mut emitter = YamlEmitter::new(&mut out);
        emitter.dump(&self.doc)?;
        std::fs::write(&self.path, out)?;
        Ok(())
    }

    pub fn root(&self) -> &Yaml {
        &self.doc
    }

    pub fn root_mut(&mut self) -> &mut Yaml {
        &mut self.doc
    }

    pub fn build_tree(&self) -> TreeNode {
        let root_path = NodePath(Vec::new());
        build_tree_node(&root_path, "".to_string(), self.root())
    }

    pub fn edit_value(&mut self, path: &NodePath, value: ScalarValue) -> Result<()> {
        let node = get_node_mut(self.root_mut(), path)?;
        *node = scalar_to_yaml(value);
        Ok(())
    }

    pub fn rename_key(&mut self, path: &NodePath, new_key: &str) -> Result<()> {
        let (parent, old_key) = split_parent_key(path)?;
        let parent_node = get_node_mut(self.root_mut(), &parent)?;
        match parent_node {
            Yaml::Hash(map) => {
                let mut existing_keys = HashSet::new();
                for (k, _) in map.iter() {
                    if let Some(key_str) = yaml_key_to_string(k) {
                        existing_keys.insert(key_str);
                    }
                }
                if existing_keys.contains(new_key) {
                    return Err(anyhow!("Key already exists"));
                }
                let mut removed = None;
                for (k, v) in map.iter() {
                    if yaml_key_to_string(k).as_deref() == Some(&old_key) {
                        removed = Some((k.clone(), v.clone()));
                        break;
                    }
                }
                if let Some((old_key_node, value)) = removed {
                    map.remove(&old_key_node);
                    map.insert(Yaml::String(new_key.to_string()), value);
                    Ok(())
                } else {
                    Err(anyhow!("Key not found"))
                }
            }
            _ => Err(anyhow!("Parent is not a mapping")),
        }
    }

    pub fn add_mapping_child(
        &mut self,
        path: &NodePath,
        key: &str,
        value: ScalarValue,
    ) -> Result<()> {
        let node = get_node_mut(self.root_mut(), path)?;
        match node {
            Yaml::Hash(map) => {
                let new_key = Yaml::String(key.to_string());
                if map.contains_key(&new_key) {
                    return Err(anyhow!("Key already exists"));
                }
                map.insert(new_key, scalar_to_yaml(value));
                Ok(())
            }
            _ => Err(anyhow!("Node is not a mapping")),
        }
    }

    pub fn add_sequence_value(&mut self, path: &NodePath, value: ScalarValue) -> Result<()> {
        let node = get_node_mut(self.root_mut(), path)?;
        match node {
            Yaml::Array(seq) => {
                seq.push(scalar_to_yaml(value));
                Ok(())
            }
            _ => Err(anyhow!("Node is not a sequence")),
        }
    }

    /// Push an empty map to the sequence at path; returns the path of the new element.
    /// Use when the user wants to add a new "object" (key-value pair) to a list.
    pub fn add_sequence_empty_map(&mut self, path: &NodePath) -> Result<NodePath> {
        let node = get_node_mut(self.root_mut(), path)?;
        match node {
            Yaml::Array(seq) => {
                let empty = YamlLoader::load_from_str("{}")?
                    .into_iter()
                    .next()
                    .unwrap_or(Yaml::Null);
                seq.push(empty);
                Ok(path.child_index(seq.len() - 1))
            }
            _ => Err(anyhow!("Node is not a sequence")),
        }
    }

    /// Convert the node at path to an empty map so child keys can be added.
    /// Use when the node is null or scalar and the user wants to add children.
    pub fn convert_to_empty_map(&mut self, path: &NodePath) -> Result<()> {
        let node = get_node_mut(self.root_mut(), path)?;
        let empty = YamlLoader::load_from_str("{}")?
            .into_iter()
            .next()
            .unwrap_or(Yaml::Null);
        *node = empty;
        Ok(())
    }

    pub fn delete_node(&mut self, path: &NodePath) -> Result<()> {
        if path.0.is_empty() {
            return Err(anyhow!("Cannot delete root"));
        }
        let (parent, last) = split_parent(path);
        let parent_node = get_node_mut(self.root_mut(), &parent)?;
        match (parent_node, last) {
            (Yaml::Hash(map), PathSegment::Key(key)) => {
                let key_node = Yaml::String(key);
                map.remove(&key_node);
                Ok(())
            }
            (Yaml::Array(seq), PathSegment::Index(index)) => {
                if index < seq.len() {
                    seq.remove(index);
                    Ok(())
                } else {
                    Err(anyhow!("Index out of bounds"))
                }
            }
            _ => Err(anyhow!("Invalid delete target")),
        }
    }
}

fn build_tree_node(path: &NodePath, key: String, node: &Yaml) -> TreeNode {
    match node {
        Yaml::Hash(map) => {
            let mut children = Vec::new();
            for (k, v) in map.iter() {
                let key_str = yaml_key_to_string(k).unwrap_or_else(|| "<non-string>".to_string());
                let child_path = path.child_key(&key_str);
                children.push(build_tree_node(&child_path, key_str, v));
            }
            TreeNode {
                path: path.clone(),
                key,
                node_type: NodeType::Map,
                value_preview: String::new(),
                children,
            }
        }
        Yaml::Array(seq) => {
            let mut children = Vec::new();
            for (idx, item) in seq.iter().enumerate() {
                let child_path = path.child_index(idx);
                let display_key = display_key_for_yaml(item);
                children.push(build_tree_node(&child_path, display_key, item));
            }
            TreeNode {
                path: path.clone(),
                key,
                node_type: NodeType::Seq,
                value_preview: String::new(),
                children,
            }
        }
        _ => TreeNode {
            path: path.clone(),
            key,
            node_type: yaml_node_type(node),
            value_preview: scalar_preview(node),
            children: Vec::new(),
        },
    }
}

fn yaml_key_to_string(key: &Yaml) -> Option<String> {
    match key {
        Yaml::String(value) => Some(value.clone()),
        _ => None,
    }
}

/// Display label for an array element: first key if object, else value preview. No index (0, 1, ...).
fn display_key_for_yaml(node: &Yaml) -> String {
    match node {
        Yaml::Hash(map) => map
            .iter()
            .next()
            .and_then(|(k, _)| yaml_key_to_string(k))
            .unwrap_or_else(|| "{}".to_string()),
        Yaml::Array(seq) => seq
            .first()
            .map(|first| display_key_for_yaml(first))
            .unwrap_or_else(|| "[]".to_string()),
        _ => {
            let preview = scalar_preview(node);
            if preview.len() > 40 {
                format!("{}…", preview.chars().take(39).collect::<String>())
            } else {
                preview
            }
        }
    }
}

pub fn yaml_node_type(node: &Yaml) -> NodeType {
    match node {
        Yaml::Hash(_) => NodeType::Map,
        Yaml::Array(_) => NodeType::Seq,
        Yaml::String(_) => NodeType::String,
        Yaml::Integer(_) | Yaml::Real(_) => NodeType::Number,
        Yaml::Boolean(_) => NodeType::Bool,
        Yaml::Null => NodeType::Null,
        _ => NodeType::Unknown,
    }
}

pub fn scalar_preview(node: &Yaml) -> String {
    match node {
        Yaml::String(value) => format!("\"{}\"", escape_yaml_string(value)),
        Yaml::Integer(value) => value.to_string(),
        Yaml::Real(value) => value.clone(),
        Yaml::Boolean(value) => value.to_string(),
        Yaml::Null => "null".to_string(),
        _ => String::new(),
    }
}

pub fn escape_yaml_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

pub fn unescape_yaml_string(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScalarValue {
    String(String),
    Bool(bool),
    Null,
    Number(ScalarNumber),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScalarNumber {
    Integer(i64),
    Float(f64),
}

pub fn parse_scalar_input(input: &str) -> Result<ScalarValue> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(ScalarValue::Null);
    }
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        return Ok(ScalarValue::String(unescape_yaml_string(inner)));
    }
    let lower = trimmed.to_lowercase();
    match lower.as_str() {
        "true" => return Ok(ScalarValue::Bool(true)),
        "false" => return Ok(ScalarValue::Bool(false)),
        "null" => return Ok(ScalarValue::Null),
        _ => {}
    }
    if let Ok(value) = trimmed.parse::<i64>() {
        return Ok(ScalarValue::Number(ScalarNumber::Integer(value)));
    }
    if let Ok(value) = trimmed.parse::<f64>() {
        return Ok(ScalarValue::Number(ScalarNumber::Float(value)));
    }
    // YAML allows unquoted strings; treat remaining input as string
    Ok(ScalarValue::String(trimmed.to_string()))
}

fn scalar_to_yaml(value: ScalarValue) -> Yaml {
    match value {
        ScalarValue::String(value) => Yaml::String(value),
        ScalarValue::Bool(value) => Yaml::Boolean(value),
        ScalarValue::Null => Yaml::Null,
        ScalarValue::Number(ScalarNumber::Integer(value)) => Yaml::Integer(value),
        ScalarValue::Number(ScalarNumber::Float(value)) => Yaml::Real(value.to_string()),
    }
}

fn get_node_mut<'a>(root: &'a mut Yaml, path: &NodePath) -> Result<&'a mut Yaml> {
    let mut node = root;
    for segment in &path.0 {
        match segment {
            PathSegment::Key(key) => match node {
                Yaml::Hash(map) => {
                    let key_node = Yaml::String(key.clone());
                    node = map.get_mut(&key_node).ok_or_else(|| anyhow!("Key not found"))?;
                }
                _ => return Err(anyhow!("Expected mapping")),
            },
            PathSegment::Index(index) => match node {
                Yaml::Array(seq) => {
                    node = seq.get_mut(*index).ok_or_else(|| anyhow!("Index out of bounds"))?;
                }
                _ => return Err(anyhow!("Expected sequence")),
            },
        }
    }
    Ok(node)
}

fn split_parent(path: &NodePath) -> (NodePath, PathSegment) {
    let mut parent = path.0.clone();
    let last = parent.pop().expect("path not empty");
    (NodePath(parent), last)
}

fn split_parent_key(path: &NodePath) -> Result<(NodePath, String)> {
    let (parent, last) = split_parent(path);
    match last {
        PathSegment::Key(key) => Ok((parent, key)),
        _ => Err(anyhow!("Not a mapping key")),
    }
}

pub fn flatten_visible(
    node: &TreeNode,
    expanded: &HashSet<String>,
    filter: Option<&str>,
) -> Vec<VisibleRow> {
    let mut rows = Vec::new();
    let query = filter.map(|q| q.to_lowercase());
    let mut ancestors = HashSet::new();
    if let Some(q) = &query {
        collect_matching_ancestors(node, q, &mut ancestors);
    }
    walk_visible(node, expanded, query.as_deref(), &ancestors, 0, &mut rows);
    rows
}

fn collect_matching_ancestors(node: &TreeNode, query: &str, ancestors: &mut HashSet<String>) -> bool {
    let mut matched = node_matches(node, query);
    for child in &node.children {
        if collect_matching_ancestors(child, query, ancestors) {
            matched = true;
        }
    }
    if matched && !node.path.0.is_empty() {
        ancestors.insert(node.path.dot_path());
    }
    matched
}

fn walk_visible(
    node: &TreeNode,
    expanded: &HashSet<String>,
    query: Option<&str>,
    ancestors: &HashSet<String>,
    depth: usize,
    rows: &mut Vec<VisibleRow>,
) {
    // Show root as a selectable row when it's a Map or Seq so user can add top-level keys/items.
    if node.path.0.is_empty()
        && matches!(node.node_type, NodeType::Map | NodeType::Seq)
    {
        rows.push(VisibleRow {
            path: node.path.clone(),
            depth: 0,
            display_key: "(root)".to_string(),
            display_value_preview: String::new(),
            node_type: node.node_type.clone(),
            is_container: true,
        });
    }
    if !node.path.0.is_empty() {
        if let Some(q) = query {
            let dot = node.path.dot_path();
            if !node_matches(node, q) && !ancestors.contains(&dot) {
                return;
            }
        }
        rows.push(VisibleRow {
            path: node.path.clone(),
            depth,
            display_key: node.key.clone(),
            display_value_preview: node.value_preview.clone(),
            node_type: node.node_type.clone(),
            is_container: matches!(node.node_type, NodeType::Map | NodeType::Seq),
        });
    }

    let should_expand = if let Some(_q) = query {
        if node.path.0.is_empty() {
            true
        } else {
            ancestors.contains(&node.path.dot_path())
        }
    } else {
        node.path.0.is_empty() || expanded.contains(&node.path.dot_path())
    };

    if should_expand {
        for child in &node.children {
            walk_visible(child, expanded, query, ancestors, depth + 1, rows);
        }
    }
}

fn node_matches(node: &TreeNode, query: &str) -> bool {
    let query = query.to_lowercase();
    let dot = node.path.dot_path().to_lowercase();
    dot.contains(&query) || node.key.to_lowercase().contains(&query)
}

pub fn visible_row_by_path(rows: &[VisibleRow], path: &NodePath) -> Option<usize> {
    rows.iter()
        .position(|row| row.path == *path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn dot_path_generation() {
        let path = NodePath(vec![
            PathSegment::Key("items".into()),
            PathSegment::Index(0),
            PathSegment::Key("name".into()),
        ]);
        assert_eq!(path.dot_path(), "items.0.name");
    }

    #[test]
    fn depth_computation() {
        let path = NodePath(vec![
            PathSegment::Key("server".into()),
            PathSegment::Key("tls".into()),
            PathSegment::Key("enabled".into()),
        ]);
        assert_eq!(path.depth(), 3);
    }

    #[test]
    fn scalar_parsing_rules() {
        assert_eq!(
            parse_scalar_input("\"hello\"").unwrap(),
            ScalarValue::String("hello".into())
        );
        assert_eq!(parse_scalar_input("true").unwrap(), ScalarValue::Bool(true));
        assert_eq!(parse_scalar_input("null").unwrap(), ScalarValue::Null);
        assert_eq!(
            parse_scalar_input("42").unwrap(),
            ScalarValue::Number(ScalarNumber::Integer(42))
        );
        assert_eq!(
            parse_scalar_input("3.14").unwrap(),
            ScalarValue::Number(ScalarNumber::Float(3.14))
        );
        assert_eq!(
            parse_scalar_input("hello").unwrap(),
            ScalarValue::String("hello".into())
        );
        assert_eq!(parse_scalar_input("").unwrap(), ScalarValue::Null);
        assert_eq!(parse_scalar_input("   ").unwrap(), ScalarValue::Null);
    }
}
