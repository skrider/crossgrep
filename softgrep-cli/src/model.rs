use anyhow::{bail, Result};
use tokenizers::tokenizer::Tokenizer;

#[derive(Clone, Copy, Debug)]
pub enum Model {
    CodeBert,
    // wide model for testing purposes
    Noop,
}

impl Model {
    pub fn from_pretrained(identifier: &str) -> Result<Self> {
        match identifier {
            "codebert" => Ok(Model::CodeBert),
            _ => bail!("unsupported model: {}", identifier),
        }
    }

    pub fn prepare_input_ids(&self, input_ids: &mut Vec<u32>, ids: &[u32]) {
        match self {
            Model::CodeBert => {
                assert!(ids.len() <= self.chunk_size() - self.special_tokens());
                input_ids.push(0);
                for i in ids {
                    input_ids.push(*i);
                }
                input_ids.push(2);
                for _ in 0..(self.chunk_size() - self.special_tokens() - ids.len()) {
                    input_ids.push(1);
                }
                assert!(input_ids.len() == self.chunk_size());
            }
            Model::Noop => {
                input_ids.clone_from_slice(ids);
            }
        }
    }

    pub fn chunk_size(&self) -> usize {
        match self {
            Model::CodeBert => 512,
            Model::Noop => usize::MAX,
        }
    }

    pub fn chunk_overlap(&self) -> usize {
        match self {
            Model::CodeBert => 64,
            Model::Noop => 0,
        }
    }

    pub fn special_tokens(&self) -> usize {
        match self {
            Model::CodeBert => 2,
            Model::Noop => 0,
        }
    }

    // TODO cache/share this for when there are multiple extractors
    pub fn tokenizer(&self) -> Tokenizer {
        Tokenizer::from_pretrained(
            match self {
                Model::CodeBert => "roberta-base",
                Model::Noop => "roberta-base",
            },
            None,
        )
        .expect("could not load tokenizer")
    }
}
