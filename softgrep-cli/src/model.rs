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

    pub fn postprocess_tokens(&self, tokens: &mut Vec<u32>) {
        match self {
            Model::CodeBert => {
                tokens.insert(0, 0);
                tokens.push(2);
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
