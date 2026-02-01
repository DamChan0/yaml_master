use crate::yaml_model::VisibleRow;

pub fn matches_row(row: &VisibleRow, query: &str) -> bool {
    let q = query.to_lowercase();
    row.path.dot_path().to_lowercase().contains(&q)
        || row.display_key.to_lowercase().contains(&q)
}

pub fn next_match(matches: &[usize], current: usize) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let pos = matches.iter().position(|&idx| idx == current);
    match pos {
        Some(i) if i + 1 < matches.len() => Some(matches[i + 1]),
        _ => Some(matches[0]),
    }
}

pub fn prev_match(matches: &[usize], current: usize) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let pos = matches.iter().position(|&idx| idx == current);
    match pos {
        Some(0) | None => matches.last().copied(),
        Some(i) => Some(matches[i - 1]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml_model::{NodePath, PathSegment, VisibleRow, NodeType};
    use pretty_assertions::assert_eq;

    fn row(path: &str, key: &str) -> VisibleRow {
        let path = NodePath(
            path.split('.')
                .filter(|s| !s.is_empty())
                .map(|seg| {
                    if let Ok(idx) = seg.parse::<usize>() {
                        PathSegment::Index(idx)
                    } else {
                        PathSegment::Key(seg.to_string())
                    }
                })
                .collect(),
        );
        VisibleRow {
            path,
            depth: 0,
            display_key: key.to_string(),
            display_value_preview: String::new(),
            node_type: NodeType::String,
            is_container: false,
        }
    }

    #[test]
    fn match_logic() {
        let row = row("server.tls.enabled", "enabled");
        assert!(matches_row(&row, "tls"));
        assert!(matches_row(&row, "enabled"));
        assert!(!matches_row(&row, "missing"));
    }

    #[test]
    fn next_prev_navigation() {
        let matches = vec![1, 3, 5];
        assert_eq!(next_match(&matches, 1), Some(3));
        assert_eq!(next_match(&matches, 5), Some(1));
        assert_eq!(prev_match(&matches, 1), Some(5));
        assert_eq!(prev_match(&matches, 3), Some(1));
    }
}
