use serde_derive::{Serialize, Deserialize};
use std::{fmt, fs, io};
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[error(transparent)]
    ParseError(ParseError),

    #[error(transparent)]
    YamlError(#[from] serde_yaml::Error),

    // #[error(transparent)]
    // TomlSerError(#[from] toml::ser::Error),
    #[error(transparent)]
    IOError(#[from] io::Error),
    // #[error(transparent)]
    // AtomicIOError(#[from] atomicwrites::Error<io::Error>),

    #[error(transparent)]
    BraceExpansionError(#[from] BraceExpansionError),
}

#[derive(Error, Debug)]
pub struct ParseError {
    pub filename: String,
    pub message: String,
}

#[derive(Error, Debug, PartialEq)]
#[error("Failed to perform brace expansion to {str}: {message}")]
pub struct BraceExpansionError {
    pub str: String,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to parse {}: {}", self.filename, self.message)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialOrd, PartialEq)]
pub struct PortName(pub String);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct NamedLink {
    pub src: PortName,
    pub dst: PortName,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NamedLinks(pub Vec<NamedLink>);

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub links: NamedLinks,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            links: NamedLinks(vec![]),
        }
    }
}

fn is_numeric_string(str: &str) -> bool {
    for ch in str.chars() {
	if !ch.is_numeric() {
	    return false
	}
    }
    true
}

fn brace_expansion(str: &str) -> Result<Vec<String>, BraceExpansionError>
{
    // Performs simple brace expansion for enumerations and ranges,
    // but not both, and not recursively. See tests for examples.
    let mut result = vec!["".to_string()];
    #[derive(PartialEq)]
    enum State {
	BeforeBrace,
	BraceOpen, // todo: it could be cool to store expansions inside this enum..
	BraceOpenDot,
	RangeOpen,
	AfterBrace,
    }
    let mut expansions = vec![];
    let mut state = State::BeforeBrace;
    for ch in str.chars() {
	match (&state, ch) {
	    (State::BeforeBrace, '{') => {
		state = State::BraceOpen;
		expansions.push("".to_string());
	    }
	    (State::AfterBrace, '{') => {
		return Err(BraceExpansionError {
		    str: str.to_string(),
		    message: "Can only have one opening brace to expand".to_string()
		});
	    }
	    (State::BeforeBrace, '}') => {
		return Err(BraceExpansionError {
		    str: str.to_string(),
		    message: "Cannot have closing brace before opening brace".to_string()
		});
	    }
	    (State::BeforeBrace | State::AfterBrace, _) => {
		for res in &mut result {
		    res.push(ch);
		}
	    }
	    (State::BraceOpen, ',') => {
		expansions.push("".to_string());
		result.push(result.first().expect("There is always at least one element in results when reaching this state").clone());
	    }
	    (State::BraceOpen, '.') => {
		if expansions.len() == 1 && is_numeric_string(&expansions[0]) {
		    state = State::BraceOpenDot;
		} else {
		    expansions
			.last_mut()
			.expect("There is always at least one element in expansions when reaching this state")
			.push(ch);
		}
	    }
	    (State::BraceOpenDot, '.') => {
		state = State::RangeOpen;
		expansions.push("".to_string());
	    }
	    (State::BraceOpen, '{') => {
		return Err(BraceExpansionError {
		    str: str.to_string(),
		    message: "Cannot open brace within an open brace".to_string()
		});
	    }
	    (State::RangeOpen, '}') => {
		assert_eq!(expansions.len(), 2);
		let range_begin: i64 = expansions[0].parse().map_err(
		    |_| BraceExpansionError {
			str: str.to_string(),
			message: "Cannot parse range begin".to_string()
		    })?;
		let range_end: i64 = expansions[1].parse().map_err(
		    |_| BraceExpansionError {
			str: str.to_string(),
			message: "Cannot parse range end".to_string()
		    })?;
		if range_begin > range_end {
		    return Err(BraceExpansionError {
			str: str.to_string(),
			message: "Ranges must be increasing".to_string()
		    })
		}
		let first_result = result.first().expect("There is always at least one element in results when reaching this state").clone();
		for value in (range_begin + 1)..=range_end {
		    result.push(format!("{0}{1}", first_result.clone(), value));
		}
		result.first_mut().unwrap().push_str(&format!("{0}", range_begin));
		expansions.clear();
		state = State::AfterBrace;
	    }
	    (State::BraceOpen, '}') => {
		assert_eq!(result.len(), expansions.len());
		for (res, exp) in std::iter::zip(&mut result, &expansions) {
		    res.push_str(&exp);
		}
		expansions.clear();
		state = State::AfterBrace;
	    }
	    (State::RangeOpen, x) => {
		if !x.is_numeric() {
		    return Err(BraceExpansionError {
			str: str.to_string(),
			message: "Range must be numeric".to_string()
		    });
		}
		expansions
		    .last_mut()
		    .expect("There is always at least one element in expansions when reaching this state")
		    .push(x);
		state = State::RangeOpen;
	    }
	    (State::BraceOpenDot, _) => {
		expansions
		    .last_mut()
		    .expect("There is always at least one element in expansions when reaching this state")
		    .push('.');
		expansions
		    .last_mut()
		    .expect("There is always at least one element in expansions when reaching this state")
		    .push(ch);
	    }
	    (State::BraceOpen, _) => {
		expansions
		    .last_mut()
		    .expect("There is always at least one element in expansions when reaching this state")
		    .push(ch);
	    }
	}
    }
    if state == State::BeforeBrace || state == State::AfterBrace {
	Ok(result)
    } else {
	return Err(BraceExpansionError {
	    str: str.to_string(),
	    message: "Must close open brace".to_string()
	});
    }
}

fn expand_links(links: NamedLinks) -> Result<NamedLinks, Error> {
    let mut new_links = NamedLinks::default();
    for link in links.0.iter() {
	let src_expansions = brace_expansion(&link.src.0)?;
	let dst_expansions = brace_expansion(&link.dst.0)?;
	if src_expansions.len() != dst_expansions.len() {
	    return Err(Error::from(BraceExpansionError { str: format!("{0} and {1}",
								      link.src.0.clone(),
								      link.dst.0.clone()),
							 message: "Number of expansions need to match".to_string() }))?;
	}
	for (src, dst) in std::iter::zip(src_expansions.iter(),
					 dst_expansions.iter()) {
	    new_links.0.push(NamedLink {src: PortName(src.clone()),
					dst: PortName(dst.clone())});
	}
    }
    Ok(new_links)
}

impl Config {
    // If no file is found, returns default config instead of error
    pub fn load(filename: &str) -> Result<Config, Error> {
        let contents = match fs::read_to_string(filename) {
            Ok(contents) => contents,
            // Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(error) => return Err(Error::IOError(error)),
        };
        let mut config: Config = match serde_yaml::from_str(&contents) {
            Ok(contents) => contents,
            Err(error) if error.location().is_some() => {
                return Err(Error::ParseError(ParseError {
                    filename: String::from(filename),
                    message: format!("{}", error),
                }));
            }
            Err(error) => return Err(Error::YamlError(error)),
        };
	config.links = expand_links(config.links)?;
        Ok(config)
    }

    pub fn dump(&self) {
	println!("{}", serde_yaml::to_string(&self).expect("Failed to serialize yaml"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion() {
        assert_eq!(brace_expansion(""), Ok(vec!["".to_string()]));
        assert_eq!(brace_expansion("a"), Ok(vec!["a".to_string()]));
        assert_eq!(brace_expansion("a{"),
		   Err(BraceExpansionError {
		       str: "a{".to_string(),
		       message: "Must close open brace".to_string()
		   }));
        assert_eq!(brace_expansion("a}"),
		   Err(BraceExpansionError {
		       str: "a}".to_string(),
		       message: "Cannot have closing brace before opening brace".to_string()
		   }));
        assert_eq!(brace_expansion("a{{"),
		   Err(BraceExpansionError {
		       str: "a{{".to_string(),
		       message: "Cannot open brace within an open brace".to_string()
		   }));
        assert_eq!(brace_expansion("a}"),
		   Err(BraceExpansionError {
		       str: "a}".to_string(),
		       message: "Cannot have closing brace before opening brace".to_string()
		   }));
        assert_eq!(brace_expansion("a{b"),
		   Err(BraceExpansionError {
		       str: "a{b".to_string(),
		       message: "Must close open brace".to_string()
		   }));
        assert_eq!(brace_expansion("a{b,"),
		   Err(BraceExpansionError {
		       str: "a{b,".to_string(),
		       message: "Must close open brace".to_string()
		   }));
        assert_eq!(brace_expansion("a{}"),
		   Ok(vec!["a".to_string()]));
        assert_eq!(brace_expansion("a{b}"),
		   Ok(vec!["ab".to_string()]));
        assert_eq!(brace_expansion("a{b,c}"),
		   Ok(vec![
		       "ab".to_string(),
		       "ac".to_string(),
		   ]));
        assert_eq!(brace_expansion("a{b,c}d"),
		   Ok(vec![
		       "abd".to_string(),
		       "acd".to_string(),
		   ]));
        assert_eq!(brace_expansion("a{b,cc,a}d"),
		   Ok(vec![
		       "abd".to_string(),
		       "accd".to_string(),
		       "aad".to_string(),
		   ]));
        assert_eq!(brace_expansion("a{}b"),
		   Ok(vec!["ab".to_string()]));
        assert_eq!(brace_expansion("a{}b{"),
		   Err(BraceExpansionError {
		       str: "a{}b{".to_string(),
		       message: "Can only have one opening brace to expand".to_string()
		   }));
        assert_eq!(brace_expansion("a{0..0}"),
		   Ok(vec![
		       "a0".to_string(),
		   ]));
        assert_eq!(brace_expansion("a{0..1}"),
		   Ok(vec![
		       "a0".to_string(),
		       "a1".to_string(),
		   ]));
        assert_eq!(brace_expansion("a{1..3}"),
		   Ok(vec![
		       "a1".to_string(),
		       "a2".to_string(),
		       "a3".to_string(),
		   ]));
        assert_eq!(brace_expansion("a{1,2..3}"),
		   Ok(vec![
		       "a1".to_string(),
		       "a2..3".to_string(),
		   ]));
        assert_eq!(brace_expansion("a{2..3,4}"),
		   Err(BraceExpansionError {
		       str: "a{2..3,4}".to_string(),
		       message: "Range must be numeric".to_string()
		   }));
    }
}
