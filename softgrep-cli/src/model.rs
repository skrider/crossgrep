use anyhow::{bail, Result};
use tokenizers::tokenizer::Tokenizer;

pub enum Model {
    CodeBert,
}

impl Model {
    pub fn from_str(identifier: &str) -> Result<Self> {
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
                for i in 0..(self.chunk_size() - self.special_tokens() - ids.len()) {
                    input_ids.push(1);
                }
                assert!(input_ids.len() == self.chunk_size());
            }
        }
    }

    pub fn chunk_size(&self) -> usize {
        match self {
            Model::CodeBert => 512,
        }
    }

    pub fn chunk_overlap(&self) -> usize {
        match self {
            Model::CodeBert => 64,
        }
    }

    pub fn special_tokens(&self) -> usize {
        match self {
            Model::CodeBert => 2,
        }
    }

    // TODO cache/share this for when there are multiple extractors
    pub fn tokenizer(&self) -> Tokenizer {
        Tokenizer::from_pretrained(
            match self {
                Model::CodeBert => "microsoft/codebert-base",
            },
            None,
        )
        .expect("could not load tokenizer")
    }
}
