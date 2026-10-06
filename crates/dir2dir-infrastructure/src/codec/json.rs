//! [`TreeCodec`] for a `json2dir`-compatible JSON dialect.
//!
//! Conversion scheme (a superset of json2dir):
//!
//! | JSON                          | filesystem                     |
//! |-------------------------------|--------------------------------|
//! | `{}` / object                 | directory                     |
//! | `"string"`                    | file, UTF-8 content           |
//! | `["link", "target"]`          | symbolic link                  |
//! | `["script", "#!/bin/sh …"]`   | executable file                |
//! | `["b64", "…"]`                | binary file (base64 content)   |
//! | `["xb64", "…"]`               | executable binary file         |
//!
//! The `b64`/`xb64` tags are the extension: json2dir cannot represent
//! non-UTF-8 content at all. Non-exec permission bits are not representable
//! in this format (same as json2dir); use the native copy path for those.

use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use dir2dir_application::ports::TreeCodec;
use dir2dir_application::CodecError;
use dir2dir_domain::{FileMode, FsTree, Node, NodeName};
use serde::{Deserialize, Serialize};

/// Serializes trees to and from the JSON dialect described above.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonCodec;

/// Document-level node. Untagged, so the JSON *shape* carries the type:
/// objects are directories, strings are files, arrays are tagged pairs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum JsonNode {
    Dir(BTreeMap<String, JsonNode>),
    File(String),
    Pair(Vec<String>),
}

impl TreeCodec for JsonCodec {
    fn serialize(&self, tree: &FsTree) -> Result<String, CodecError> {
        let document = to_json_node(tree.root())?;
        serde_json::to_string_pretty(&document).map_err(|e| CodecError::Backend {
            message: e.to_string(),
        })
    }

    fn deserialize(&self, document: &str) -> Result<FsTree, CodecError> {
        let parsed: JsonNode =
            serde_json::from_str(document).map_err(|e| CodecError::InvalidDocument {
                message: e.to_string(),
            })?;
        match from_json_node(parsed)? {
            Node::Dir { children } => Ok(FsTree::from_children(children)),
            _ => Err(CodecError::InvalidDocument {
                message: "the document root must be an object".to_owned(),
            }),
        }
    }
}

fn to_json_node(node: &Node) -> Result<JsonNode, CodecError> {
    match node {
        Node::Dir { children } => {
            let mut map = BTreeMap::new();
            for (name, child) in children {
                map.insert(name.as_str().to_owned(), to_json_node(child)?);
            }
            Ok(JsonNode::Dir(map))
        }
        Node::File { content, mode } => {
            if let Ok(text) = std::str::from_utf8(content) {
                if mode.is_executable() {
                    Ok(JsonNode::Pair(vec!["script".to_owned(), text.to_owned()]))
                } else {
                    Ok(JsonNode::File(text.to_owned()))
                }
            } else {
                let tag = if mode.is_executable() { "xb64" } else { "b64" };
                Ok(JsonNode::Pair(vec![tag.to_owned(), BASE64.encode(content)]))
            }
        }
        Node::Symlink { target } => {
            let target = target.to_str().ok_or_else(|| CodecError::InvalidModel {
                message: format!("symlink target is not valid UTF-8: {target:?}"),
            })?;
            Ok(JsonNode::Pair(vec!["link".to_owned(), target.to_owned()]))
        }
    }
}

fn from_json_node(node: JsonNode) -> Result<Node, CodecError> {
    match node {
        JsonNode::Dir(map) => {
            let mut children = BTreeMap::new();
            for (key, value) in map {
                let name =
                    NodeName::new(key.clone()).map_err(|reason| CodecError::InvalidModel {
                        message: format!("invalid node name {key:?}: {reason}"),
                    })?;
                children.insert(name, from_json_node(value)?);
            }
            Ok(Node::Dir { children })
        }
        JsonNode::File(text) => Ok(Node::file(text, FileMode::DEFAULT)),
        JsonNode::Pair(parts) => {
            if parts.len() != 2 {
                return Err(CodecError::InvalidDocument {
                    message: format!(
                        "array nodes must have exactly two elements, got {}",
                        parts.len()
                    ),
                });
            }
            let mut iter = parts.into_iter();
            let tag = iter.next().expect("checked length");
            let argument = iter.next().expect("checked length");
            match tag.as_str() {
                "link" => Ok(Node::symlink(argument)),
                "script" => Ok(Node::file(argument, FileMode::EXECUTABLE)),
                "b64" => decode(&argument, FileMode::DEFAULT),
                "xb64" => decode(&argument, FileMode::EXECUTABLE),
                other => Err(CodecError::InvalidDocument {
                    message: format!(
                        "unknown array tag {other:?}; expected one of \
                         \"link\", \"script\", \"b64\", \"xb64\""
                    ),
                }),
            }
        }
    }
}

fn decode(argument: &str, mode: FileMode) -> Result<Node, CodecError> {
    let content = BASE64
        .decode(argument)
        .map_err(|e| CodecError::InvalidDocument {
            message: format!("invalid base64 content: {e}"),
        })?;
    Ok(Node::file(content, mode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dir2dir_domain::NodeName;

    fn path(s: &str) -> dir2dir_domain::NodePath {
        s.split('/')
            .map(|p| NodeName::new(p).expect("valid name"))
            .collect::<Vec<_>>()
            .into()
    }

    fn tree(entries: &[(&str, Node)]) -> FsTree {
        let mut tree = FsTree::empty();
        for (p, node) in entries {
            tree.insert(&path(p), node.clone()).expect("insert");
        }
        tree
    }

    // The example document from the json2dir README, verbatim.
    const JSON2DIR_EXAMPLE: &str = r##"{
  "greeting": "Hello, world!",
  "dir": {
    "subfile": "Content.\n",
    "subdir": {}
  },
  "symlink": ["link", "target path"],
  "script": ["script", "#!/bin/sh\necho Howdy!"]
}"##;

    #[test]
    fn parses_the_json2dir_example() {
        let tree = JsonCodec.deserialize(JSON2DIR_EXAMPLE).expect("parse");

        assert_eq!(
            tree.get(&path("greeting")),
            Some(&Node::file("Hello, world!", FileMode::DEFAULT))
        );
        assert_eq!(
            tree.get(&path("dir/subfile")),
            Some(&Node::file("Content.\n", FileMode::DEFAULT))
        );
        assert_eq!(tree.get(&path("dir/subdir")), Some(&Node::dir()));
        assert_eq!(
            tree.get(&path("symlink")),
            Some(&Node::symlink("target path"))
        );
        assert_eq!(
            tree.get(&path("script")),
            Some(&Node::file("#!/bin/sh\necho Howdy!", FileMode::EXECUTABLE))
        );
    }

    #[test]
    fn roundtrips_the_json2dir_example() {
        let tree = JsonCodec.deserialize(JSON2DIR_EXAMPLE).expect("parse");
        let document = JsonCodec.serialize(&tree).expect("serialize");
        let reparsed = JsonCodec.deserialize(&document).expect("reparse");
        assert_eq!(tree, reparsed);
    }

    #[test]
    fn carries_binary_content_in_base64() {
        let tree = tree(&[
            (
                "blob",
                Node::file(vec![0x00, 0xff, 0xfe, 0x81], FileMode::DEFAULT),
            ),
            ("xblob", Node::file(vec![0x00, 0xff], FileMode::EXECUTABLE)),
        ]);

        let document = JsonCodec.serialize(&tree).expect("serialize");
        assert!(document.contains("\"b64\""));
        assert!(document.contains("\"xb64\""));

        let reparsed = JsonCodec.deserialize(&document).expect("reparse");
        assert_eq!(tree, reparsed);
    }

    #[test]
    fn serializes_executables_as_scripts() {
        let tree = tree(&[("run.sh", Node::file("echo hi\n", FileMode::EXECUTABLE))]);
        let document = JsonCodec.serialize(&tree).expect("serialize");
        assert!(document.contains("\"script\""));
    }

    #[test]
    fn rejects_unknown_tags_and_bad_shapes() {
        for document in [
            r#"["nope", "x"]"#,
            r#"["link"]"#,
            r#"["link", "a", "b"]"#,
            r#""just a string""#,
            r#"[]"#,
            r#"{"a/b": "c"}"#,
            r#"{"..": "c"}"#,
        ] {
            assert!(
                JsonCodec.deserialize(document).is_err(),
                "document should be rejected: {document}"
            );
        }
    }
}
