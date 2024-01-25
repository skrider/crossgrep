use tokenizers::tokenizer::Tokenizer;
use anyhow::{bail, Result};

#[derive(Debug)]
pub struct Model<'model> {
    tokenizer: &'model Tokenizer,
}

impl<'model> Model<'model> {
    pub fn from_pretrained(model_name: &str) -> Result<Self> {
        let tokenizer = Tokenizer::from_pretrained(model_name, None);

        if tokenizer.is_err() {
            bail!("Could not load model {}", model_name);
        }

        let tokenizer = tokenizer.as_ref().unwrap();

        Ok(Model { tokenizer })
    }

}
