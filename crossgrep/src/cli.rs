use crate::extractor::Extractor;
use crate::extractor_chooser::ExtractorChooser;
use crate::model::Model;
use anyhow::{bail, Context, Error, Result};
use clap::{crate_authors, crate_version, Arg, ArgAction, ArgMatches, Command};
use itertools::Itertools;
use crossgrep_sys::Language;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

pub enum Invocation {
    DoQuery(QueryOpts),
    ShowLanguages,
    ShowTree(TreeOpts),
}

#[derive(Debug)]
pub struct QueryOpts {
    pub extractors: Vec<Extractor>,
    pub paths: Vec<PathBuf>,
    pub git_ignore: bool,
    pub format: QueryFormat,
}

impl QueryOpts {
    pub fn extractor_chooser(&self) -> Result<ExtractorChooser> {
        ExtractorChooser::from_extractors(&self.extractors)
    }
}

#[derive(Debug)]
pub struct TreeOpts {
    pub language: Language,
    pub path: PathBuf,
}

impl Invocation {
    pub fn from_args(args: Vec<String>) -> Result<Self> {
        // I'm not super happy with this! I would love for LANGUAGE and QUERY to
        // be taken positionally when there is just one so we don't always have
        // to specify `-q`. However, I also want to get working on the rest of
        // the program so I'm dropping the requirement for now by making `-q`
        // required. I think that's an OK tradeoff until I can figure something
        // else better because it'll be backwards compatible with the scheme
        // I outlined above.
        //
        // Check
        // https://users.rust-lang.org/t/grep-like-argument-parsing-with-clap/63392
        // for where I asked about this in public.
        let matches = Command::new("tree-grepper")
            .version(crate_version!())
            .author(crate_authors!())
            .arg(
                Arg::new("additional-target")
                    .short('t')
                    .long("target")
                    .help("a language and tree-sitter query to restrict semantic search to")
                    .long_help("a language and tree-sitter query to restrict semantic search to")
                    .number_of_values(2)
                    .value_names(["LANGUAGE", "TARGET"])
                    .required_unless_present("languages")
                    .required_unless_present("show-tree")
                    .conflicts_with("languages")
                    .conflicts_with("show-tree")
                    .action(ArgAction::Append),
            )
            .arg(
                Arg::new("no-gitignore")
                    .long("no-gitignore")
                    .help("don't use git's ignore and exclude files to filter files")
                    .conflicts_with("languages")
                    .conflicts_with("show-tree"),
            )
            .arg(
                Arg::new("MODEL")
                    .long("model")
                    .short('m')
                    .help("model to use for embedding")
                    .conflicts_with("languages")
                    .conflicts_with("show-tree"),
            )
            .arg(
                Arg::new("PATHS")
                    .default_value(".")
                    .help("places to search for matches")
                    .num_args(1..),
            )
            .arg(
                Arg::new("FORMAT")
                    .long("format")
                    .short('f')
                    .value_parser(["lines", "json", "json-lines", "pretty-json"])
                    .default_value("lines")
                    .help("what format should we output lines in?")
                    .conflicts_with("languages")
                    .conflicts_with("show-tree"),
            )
            .arg(
                Arg::new("languages")
                    .long("languages")
                    .action(ArgAction::SetTrue)
                    .help("print the language names crossgrep knows about")
                    .conflicts_with("additional-target")
                    .conflicts_with("show-tree"),
            )
            .arg(
                Arg::new("show-tree")
                    .long("show-tree")
                    .help("Show the node names and associated text of the specified files")
                    .value_names(["LANGUAGE"])
                    .action(ArgAction::Append)
                    .conflicts_with("languages")
                    .conflicts_with("additional-target"),
            )
            .arg(
                Arg::new("QUERY")
                    .last(true)
                    .number_of_values(1)
                    .value_name("QUERY")
                    .required_unless_present("languages")
                    .required_unless_present("show-tree")
                    .conflicts_with("languages")
                    .conflicts_with("show-tree")
                    .num_args(1..)
                    .action(ArgAction::Append),
            )
            .try_get_matches_from(args)
            .context("could not parse args")?;

        if matches.get_flag("languages") {
            Ok(Self::ShowLanguages)
        } else if let Some(raw_lang) = matches.get_one::<String>("show-tree") {
            let lang = Language::from_str(raw_lang).context("could not parse language")?;

            let paths = Self::paths(&matches)?;
            if paths.len() != 1 {
                anyhow::bail!("need exactly one path to print a tree")
            }

            Ok(Self::ShowTree(TreeOpts {
                language: lang,
                path: paths[0].to_owned(),
            }))
        } else {
            Ok(Self::DoQuery(QueryOpts {
                extractors: Self::extractors(&matches)?,
                paths: Self::paths(&matches)?,
                git_ignore: !matches.contains_id("no-gitignore"),
                format: QueryFormat::from_str(
                    matches
                        .get_one::<String>("FORMAT")
                        .context("format not provided")?,
                )
                .context("could not set format")?,
            }))
        }
    }

    fn extractors(matches: &ArgMatches) -> Result<Vec<Extractor>> {
        let values = match matches.get_many::<String>("additional-target") {
            Some(values) => values,
            None => bail!("queries were required but not provided. This indicates an internal error and you should report it!"),
        };

        let model_identifier = matches
            .get_one::<String>("MODEL")
            .context("model not provided")?;
        let model = Model::from_pretrained(model_identifier).context("model not supported")?;

        // the most common case is going to be one query, so let's allocate
        // that immediately...
        let mut query_strings: HashMap<Language, String> = HashMap::with_capacity(1);

        // If you have two tree-sitter queries `(one)` and `(two)`, you can
        // join them together in a single string like `(one)(two)`. In that
        // case, the resulting query will act like an OR and match any of the
        // queries inside. Doing this automatically gives us an advantage:
        // for however many queries we get on the command line, we will only
        // ever have to run one per file, since we can combine them and you
        // can't specify queries across multiple languages! Nobody should ever
        // notice, except that they won't see as much of a slowdown for adding
        // new queries to an invocation as they might expect. (Well, hopefully!)
        for (raw_lang, raw_query) in values.tuples() {
            let lang = Language::from_str(raw_lang).context("could not parse language")?;

            let mut query_out = String::from(raw_query);

            let temp_query = lang
                .parse_query(raw_query)
                .context("could not parse query")?;

            if temp_query.capture_names().is_empty() {
                query_out.push_str("@query");
            }

            if let Some(existing) = query_strings.get_mut(&lang) {
                existing.push_str(&query_out);
            } else {
                query_strings.insert(lang, query_out);
            }
        }

        let mut out = Vec::with_capacity(query_strings.len());
        for (lang, raw_query) in query_strings {
            let query = lang
                .parse_query(&raw_query)
                .context("could not parse combined query")?;

            out.push(Extractor::new(lang, query, model))
        }

        Ok(out)
    }

    fn paths(matches: &ArgMatches) -> Result<Vec<PathBuf>> {
        match matches.get_many::<String>("PATHS") {
            Some(values) =>
                values
                    .map(|raw_path| PathBuf::from_str(raw_path).with_context(|| format!("could not parse a path from {}", raw_path)))
                    .collect(),

            None => bail!("at least one path was required but not provided. This indicates an internal errors and you should report it!"),
        }
    }
}

#[derive(Debug)]
pub enum QueryFormat {
    Lines,
    Json,
    JsonLines,
    PrettyJson,
}

impl FromStr for QueryFormat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "lines" => Ok(QueryFormat::Lines),
            "json" => Ok(QueryFormat::Json),
            "json-lines" => Ok(QueryFormat::JsonLines),
            "pretty-json" => Ok(QueryFormat::PrettyJson),
            _ => bail!("unknown format. See --help for valid formats."),
        }
    }
}
