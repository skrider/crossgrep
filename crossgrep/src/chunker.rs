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
    chunk_overlap: usize,
}

impl Chunker {
    pub fn from_model(model: Model) -> Self {
        Chunker {
            model: model.clone(),
            tokenizer: model.tokenizer(),
            chunk_size: model.chunk_size() - model.special_tokens(),
            chunk_overlap: model.chunk_overlap(),
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
            let mut tokens = vec![0; ids.len()];
            tokens.clone_from_slice(ids);

            return Ok(vec![ExtractedChunk {
                ids: tokens,
                start_byte: 0,
                end_byte: source.len(),
            }]);
        }

        let mut chunk_start = 0;
        let mut chunk_end = 0;
        let mut chunks = Vec::new();

        loop {
            chunk_end += 1;

            if chunk_end - chunk_start > self.chunk_size {
                chunk_end = std::cmp::min(ids.len(), chunk_end + self.chunk_overlap);
                let mut chunk_ids: Vec<u32> = vec![0; chunk_end - chunk_start];
                chunk_ids.clone_from_slice(&ids[chunk_start..chunk_end]);
                chunks.push(ExtractedChunk {
                    ids: chunk_ids,
                    start_byte: encoding
                        .token_to_chars(chunk_start)
                        .expect("token out of range")
                        .1
                         .0,
                    end_byte: encoding
                        .token_to_chars(chunk_start)
                        .expect("token out of range")
                        .1
                         .1,
                });

                chunk_start = chunk_end - self.chunk_overlap;
            }

            if chunk_end > ids.len() {
                break;
            }
        }

        Ok(chunks)
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtractedChunk {
    pub ids: Vec<u32>,
    pub start_byte: usize,
    pub end_byte: usize,
}
