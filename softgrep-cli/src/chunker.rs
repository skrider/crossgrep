use crate::model::Model;
use anyhow::{bail, Result};
use serde::Serialize;
use tokenizers::tokenizer::Tokenizer;
use tree_sitter::Node;

#[derive(Debug)]
pub struct Chunker {
    tokenizer: Tokenizer,
    model: Model,
    chunk_size: usize,
    inner_chunk_size: usize,
    chunk_overlap: usize,
    lookbehind_lines: usize,
}

impl Chunker {
    pub fn from_model(model: Model) -> Self {
        Chunker {
            model: model.clone(),
            tokenizer: model.tokenizer(),
            chunk_size: model.chunk_size(),
            inner_chunk_size: model.chunk_size()
                - model.chunk_overlap() * 2
                - model.special_tokens(),
            chunk_overlap: model.chunk_overlap(),
            lookbehind_lines: model.chunk_size().ilog2() as usize,
        }
    }

    pub fn chunk_node(&self, source: &[u8], node: &Node) -> Result<Vec<ExtractedChunk>> {
        assert!(source.len() == node.end_byte() - node.start_byte());

        let source_str = std::str::from_utf8(source).expect("invalid utf-8");
        let encoding = match self.tokenizer.encode(source_str, false) {
            Ok(encoding) => encoding,
            Err(err) => bail!("Could not encode source: {}", err),
        };
        let ids = encoding.get_ids();

        if ids.len() < self.chunk_size - self.model.special_tokens() {
            let mut tokens = Vec::new();
            self.model.prepare_input_ids(&mut tokens, ids);

            return Ok(vec![ExtractedChunk {
                ids: tokens,
                start_byte: 0,
                end_byte: source.len(),
            }]);
        }

        let line_ct = source
            .iter()
            .fold(0, |acc, c| if *c == '\n' as u8 { acc + 1 } else { acc });

        let mut node_terminals = vec![0; line_ct + 1];
        let start_line = node.start_position().row;
        // construct map of line numbers to nodes ending on that line
        for n in TreeWalker::from_node(node) {
            let end_line = n.end_position().row - start_line;
            let start_line = n.start_position().row - start_line;
            if start_line != end_line {
                node_terminals[end_line] += 1;
            }
        }

        let mut newline_token_indices = Vec::with_capacity(line_ct + 1);
        // sentinel newline at zero
        newline_token_indices.push(0);
        for (i, t) in source.iter().enumerate() {
            if *t == '\n' as u8 {
                let token_index = match encoding.char_to_token(i, 0) {
                    Some(i) => i,
                    None => bail!("Could not find token index for newline"),
                };

                if token_index - newline_token_indices.last().unwrap() > self.inner_chunk_size {
                    bail!("line too long");
                }

                newline_token_indices.push(token_index);
            }
        }

        let mut chunk_line_start = 0;
        let mut chunk_line_end = 0;

        let mut chunks = Vec::with_capacity(2 * ids.len() / self.chunk_size);
        let mut is_first_chunk = 1;

        while chunk_line_end < line_ct {
            chunk_line_end += 1;
            if newline_token_indices[chunk_line_end] - newline_token_indices[chunk_line_start]
                > self.chunk_size - self.chunk_overlap - is_first_chunk * self.chunk_overlap
            {
                let min_end_point =
                    std::cmp::min(chunk_line_start, chunk_line_end - self.lookbehind_lines);
                let chunk_line_end = node_terminals[min_end_point..chunk_line_end - 1]
                    .iter()
                    .enumerate()
                    .fold(
                        (0, -1),
                        |acc, (i, v)| if *v >= acc.1 { (i, *v) } else { acc },
                    )
                    .0
                    + min_end_point;

                let chunk_start = std::cmp::max(
                    0,
                    newline_token_indices[chunk_line_start] + 1 - self.chunk_overlap,
                );
                let chunk_end = std::cmp::min(
                    ids.len() - 1,
                    newline_token_indices[chunk_line_end] + self.chunk_overlap,
                );

                let mut tokens = Vec::with_capacity(self.chunk_size);
                self.model
                    .prepare_input_ids(&mut tokens, &ids[chunk_start..chunk_end]);

                let start_byte = encoding
                    .token_to_chars(chunk_start)
                    .expect("token out of range")
                    .1
                     .0;
                let end_byte = encoding
                    .token_to_chars(chunk_end)
                    .expect("token out of range")
                    .1
                     .0;

                chunks.push(ExtractedChunk {
                    ids: tokens,
                    start_byte,
                    end_byte,
                });
                eprintln!("chunk added");

                is_first_chunk = 0;
                chunk_line_start = chunk_line_end;
            }
        }

        let chunk_start = std::cmp::max(
            0,
            newline_token_indices[chunk_line_start] + 1 - self.chunk_overlap,
        );
        let chunk_end = ids.len() - 1;

        let mut tokens = Vec::with_capacity(self.chunk_size);
        self.model
            .prepare_input_ids(&mut tokens, &ids[chunk_start..chunk_end]);

        let start_byte = encoding
            .token_to_chars(chunk_start)
            .expect("token out of range")
            .1
             .0;
        let end_byte = encoding
            .token_to_chars(chunk_end)
            .expect("token out of range")
            .1
             .0;

        chunks.push(ExtractedChunk {
            ids: tokens,
            start_byte,
            end_byte,
        });

        Ok(chunks)
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtractedChunk {
    pub ids: Vec<u32>,
    pub start_byte: usize,
    pub end_byte: usize,
}

struct TreeWalker<'walker> {
    cursor: tree_sitter::TreeCursor<'walker>,
}

impl<'walker> TreeWalker<'walker> {
    fn from_node(node: &'walker Node) -> Self {
        Self {
            cursor: node.walk(),
        }
    }
}

impl<'walker> Iterator for TreeWalker<'walker> {
    type Item = tree_sitter::Node<'walker>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.goto_first_child() {
            Some(self.cursor.node())
        } else {
            while !self.cursor.goto_next_sibling() {
                if !self.cursor.goto_parent() {
                    ()
                }
            }
            Some(self.cursor.node())
        }
    }
}
