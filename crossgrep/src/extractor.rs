use anyhow::{Context, Result};
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use crossgrep_sys::Language;
use std::collections::HashSet;
use std::fmt::{self, Display};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Point, Query, QueryCursor};

use crate::chunker::{Chunker, ExtractedChunk};
use crate::model::Model;

#[derive(Debug)]
pub struct Extractor {
    language: Language,
    ts_language: tree_sitter::Language,
    query: Query,
    chunker: Chunker,
    captures: Vec<String>,
    ignores: HashSet<usize>,
}

impl Extractor {
    pub fn new(language: Language, query: Query, model: Model) -> Extractor {
        let captures = query.capture_names().to_vec();

        let mut ignores = HashSet::default();
        captures.iter().enumerate().for_each(|(i, name)| {
            if name.starts_with('_') {
                ignores.insert(i);
            }
        });

        if captures.len() == ignores.len() {
            eprintln!("Warning: query only has ignored captures. No results will be printed.");
        }

        Extractor {
            ts_language: language.language(),
            chunker: Chunker::from_model(model),
            language,
            query,
            captures,
            ignores,
        }
    }

    pub fn language(&self) -> &Language {
        &self.language
    }

    pub fn extract_from_file(
        &self,
        path: &Path,
        parser: &mut Parser,
    ) -> Result<Option<ExtractedFile>> {
        let source = fs::read(path).context("could not read file")?;

        self.extract_from_text(Some(path), &source, parser)
    }

    pub fn extract_from_text(
        &self,
        path: Option<&Path>,
        source: &[u8],
        parser: &mut Parser,
    ) -> Result<Option<ExtractedFile>> {
        parser
            .set_language(self.ts_language)
            .context("could not set language")?;

        let tree = parser
            .parse(source, None)
            // note: this could be a timeout or cancellation, but we don't set
            // that so we know it's always a language error. Buuuut we also
            // always set the language above so if this happens we also know
            // it's an internal error.
            .context(
                "could not parse to a tree. This is an internal error and should be reported.",
            )?;

        let mut cursor = QueryCursor::new();

        let extracted_matches = cursor
            .matches(&self.query, tree.root_node(), source)
            .flat_map(|query_match| query_match.captures)
            // note: the casts here could potentially break if run on a 16-bit
            // microcontroller. I don't think this is a huge problem, though,
            // since even the gnarliest queries I've written have something on
            // the order of 20 matches. Nowhere close to 2^16!
            .filter(|capture| !self.ignores.contains(&(capture.index as usize)))
            .filter_map(|capture| {
                let name = &self.captures[capture.index as usize];
                let node = capture.node;
                let node_source = &source[node.byte_range()];

                let chunks = match self.chunker.chunk_node(node_source, &node) {
                    Ok(chunks) => chunks,
                    Err(e) => {
                        eprintln!(
                            "warning: tokenization for {} failed: {}",
                            path.map(|p| p.to_path_buf().into_os_string().into_string().unwrap())
                                .unwrap_or(String::from("stdin")),
                            e
                        );
                        return None;
                    }
                };

                // we already check for valid utf-8 in chunker
                let text = std::str::from_utf8(node_source).unwrap().to_string();

                Some(ExtractedMatch {
                    kind: node.kind(),
                    name,
                    text,
                    start: node.start_position(),
                    end: node.end_position(),
                    chunks,
                })
            })
            .collect::<Vec<ExtractedMatch>>();

        if extracted_matches.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ExtractedFile {
                file: path.map(|p| p.to_owned()),
                file_type: self.language.to_string(),
                matches: extracted_matches,
            }))
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtractedFile<'query> {
    file: Option<PathBuf>,
    file_type: String,
    matches: Vec<ExtractedMatch<'query>>,
}

impl<'query> Display for ExtractedFile<'query> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: is there a better way to do this unwrapping? This implementation
        // turns non-UTF-8 paths into "NON-UTF8 FILENAME". I don't know exactly
        // what circumstances that could happen in... maybe we should just wait
        // for bug reports?
        let filename = self
            .file
            .as_ref()
            .map(|f| f.to_str().unwrap_or("NON-UTF8 FILENAME"))
            .unwrap_or("NO FILE");

        for extraction in &self.matches {
            writeln!(
                f,
                "{}:{}:{}:{}:{}",
                filename,
                extraction.start.row + 1,
                extraction.start.column + 1,
                extraction.name,
                extraction.text
            )?
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtractedMatch<'query> {
    kind: &'static str,
    name: &'query str,
    text: String,
    #[serde(serialize_with = "serialize_point")]
    start: Point,
    #[serde(serialize_with = "serialize_point")]
    end: Point,
    chunks: Vec<ExtractedChunk>,
}

fn serialize_point<S>(point: &Point, sz: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut out = sz.serialize_struct("Point", 2)?;
    out.serialize_field("row", &(point.row + 1))?;
    out.serialize_field("column", &(point.column + 1))?;
    out.end()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossgrep_sys::Language;
    use tree_sitter::Parser;

    #[test]
    fn test_matches_are_extracted() {
        let lang = Language::Elm;
        let query = lang
            .parse_query("(import_clause (upper_case_qid)@import)")
            .unwrap();
        let extractor = Extractor::new(lang, query, Model::Noop);

        let extracted = extractor
            .extract_from_text(None, b"import Html.Styled", &mut Parser::new())
            // From Result<Option<ExtractedFile>>
            .unwrap()
            // From Option<ExtractedFile>
            .unwrap();

        assert_eq!(extracted.matches.len(), 1);
        assert_eq!(extracted.matches[0].name, "import");
        assert_eq!(extracted.matches[0].text, "Html.Styled");
    }

    #[test]
    fn test_underscore_names_are_ignored() {
        let lang = Language::Elm;
        let query = lang
            .parse_query("(import_clause (upper_case_qid)@_import)")
            .unwrap();
        let extractor = Extractor::new(lang, query, Model::Noop);

        let extracted = extractor
            .extract_from_text(None, b"import Html.Styled", &mut Parser::new())
            // From Result<Option<ExtractedFile>>
            .unwrap();

        assert_eq!(extracted, None);
    }

    #[test]
    fn test_underscore_names_can_still_be_used_in_matchers() {
        let lang = Language::JavaScript;
        let query = lang
            .parse_query("(call_expression (identifier)@_fn (arguments . (string)@import .) (#eq? @_fn require))")
            .unwrap();
        let extractor = Extractor::new(lang, query, Model::Noop);

        let extracted = extractor
            .extract_from_text(None, b"let foo = require(\"foo.js\")", &mut Parser::new())
            // From Result<Option<ExtractedFile>>
            .unwrap()
            // From Option<ExtractedFile>
            .unwrap();

        assert_eq!(extracted.matches.len(), 1);
        assert_eq!(extracted.matches[0].name, "import");
        assert_eq!(extracted.matches[0].text, "\"foo.js\"");
    }
}
