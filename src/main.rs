use cmd_lib::{run_cmd, run_fun};
use regex::Regex;
use std::{
	collections::HashMap,
	env,
	io::{Read, Seek, Write},
};
use tempfile::{Builder, NamedTempFile};
use xmltree::{Element, XMLNode};

const FROM_SEGMENT_HEADER: &[&str] = &["title"];
const FROM_TAGS: &[&str] = &[
	"COMMENT",
	"ARTIST",
	"ALBUM",
	"DATE",
	"DESCRIPTION",
	"SYNOPSIS",
	"PURL",
	"PART_NUMBER",
	"TOTAL_PARTS",
];
const ORDERING: &[&str] = &[
	"title",
	"ARTIST",
	"ALBUM",
	"PART_NUMBER",
	"DESCRIPTION",
	"COMMENT",
	"PURL",
];

fn hashmap_to_format(hash: HashMap<String, String>, order: Vec<String>) -> String {
	let mut res = String::new();
	let _e = String::from("");
	for key in order
		.iter()
		.chain(hash.keys().filter(|k| !order.contains(k)))
	{
		res = format!(
			"{}(-------{}-------)\n{}\n",
			res,
			key,
			hash.get(key).unwrap_or(&_e)
		);
	}
	res
}

fn format_to_hashmap(format: String) -> HashMap<String, String> {
	let header_regex =
		Regex::new(r"\(-------(?<name>.+)-------\)").expect("Regex construction errored.");
	let mut hash: HashMap<String, String> = HashMap::new();
	let mut val = String::from("");
	let mut key: Option<String> = None;
	for line in format.split("\n") {
		if let Some(caps) = header_regex.captures(line) {
			if let Some(prev_key) = key {
				hash.insert(prev_key, String::from(val.trim()));
			}
			val = String::from("");
			key = caps.name("name").map(|m| String::from(m.as_str()));
			if let Some(test_key) = &key {
				if !FROM_TAGS.contains(&test_key.as_str())
					&& !FROM_SEGMENT_HEADER.contains(&test_key.as_str())
				{
					println!("File contained unrecognized header: '{}'", test_key);
					key = None;
				}
			}
		} else {
			val = format!("{}\n{}", val, line)
		}
	}
	hash
}

fn is_metadata_child(child: &Element) -> bool {
	child
		.get_child("Targets")
		.expect("Output from mkvextract had incorrectly formatted track data")
		.get_child("TrackUID")
		.is_none()
}

fn main() -> Result<(), String> {
	let all_keys: Vec<String> = FROM_SEGMENT_HEADER
		.iter()
		.chain(FROM_TAGS)
		.map(|s| String::from(*s))
		.collect();

	let args: Vec<String> = env::args().skip(1).collect();
	if args.is_empty() {
		return Err(String::from(
			"mkv_edit must be given at least one filename to edit",
		));
	}
	let Ok(editor) = env::var("EDITOR") else {
		return Err(String::from(
			"Please set a system editor in the EDITOR variable!",
		));
	};

	let Ok(xml_temp_file) = Builder::new().suffix(".mkv").tempfile() else {
		return Err(String::from("Could not create temporary file"));
	};
	let Some(xml_temp_file_path) = xml_temp_file.path().to_str() else {
		return Err(String::from("Could not get temporary file path!"));
	};
	let Ok(edit_temp_file) = NamedTempFile::new() else {
		return Err(String::from("Could not create temporary file"));
	};
	let Some(edit_temp_file_path) = edit_temp_file.path().to_str() else {
		return Err(String::from("Could not get temporary file path!"));
	};
	for inp_file in args {
		let Ok(_) = xml_temp_file.as_file().set_len(0) else {
			return Err(String::from("Error clearing xml temporary file"));
		};
		let Ok(_) = xml_temp_file.as_file().rewind() else {
			return Err(String::from(
				"Going back to start of xml temporary file failed",
			));
		};
		let Ok(_) = edit_temp_file.as_file().set_len(0) else {
			return Err(String::from("Error clearing plaintext temporary file"));
		};
		let Ok(_) = edit_temp_file.as_file().rewind() else {
			return Err(String::from(
				"Going back to start of plaintext temporary file failed",
			));
		};

		let mut hash: HashMap<String, String> = HashMap::new();

		for key in FROM_SEGMENT_HEADER {
			let prefix = format!("| + {}: ", key);
			hash.insert(
				String::from(*key),
				run_fun!(mkvinfo "$inp_file" | grep "$prefix" -m 1 -i).map_or(
					String::from(""),
					|val| {
						val.split_at_checked(prefix.len())
							.map_or(String::from(""), |(_, val)| String::from(val))
					},
				),
			);
		}

		let Ok(x) = run_fun!(mkvextract "$inp_file" tags) else {
			return Err(String::from(
				"Could not extract tags.  Do you have mkvextract installed?",
			));
		};

		let mut xml =
			Element::parse(x.as_bytes()).expect("Output from mkvextract was invalid xml.");

		for key in FROM_TAGS {
			hash.insert(String::from(*key), String::from(""));
		}
		for c in &xml.children {
			if let XMLNode::Element(child) = c {
				if !is_metadata_child(child) {
					continue;
				}
				for t in &child.children {
					if let XMLNode::Element(tag) = t {
						if tag.name != *"Simple" {
							continue;
						}
						let Some(key) = tag.get_child("Name").map(|c| {
							c.get_text()
								.map(|s| s.into_owned())
								.unwrap_or(String::from(""))
						}) else {
							continue;
						};
						if !FROM_TAGS.contains(&key.as_str()) {
							continue;
						}
						hash.insert(
							String::from(key.as_str()),
							tag.get_child("String")
								.map(|c| {
									c.get_text()
										.map(|s| s.into_owned())
										.unwrap_or(String::from(""))
								})
								.unwrap_or(String::from("")),
						);
					}
				}
				break;
			}
		}

		let formatted = format!(
			"{}\n{}",
			inp_file,
			hashmap_to_format(
				hash.clone(),
				ORDERING.iter().map(|s| String::from(*s)).collect(),
			)
		);
		let Ok(_) = edit_temp_file.as_file().write_all(formatted.as_bytes()) else {
			return Err(String::from(
				"Could not write formatted data to plaintext temporary file",
			));
		};
		let Ok(_) = run_cmd!("$editor" "$edit_temp_file_path") else {
			return Err(String::from("Could not open the editor"));
		};

		let mut file_data = String::new();
		let Ok(_) = edit_temp_file.as_file().rewind() else {
			return Err(String::from(
				"Going back to start of plaintext temporary file failed",
			));
		};
		let Ok(_) = edit_temp_file.as_file().read_to_string(&mut file_data) else {
			return Err(String::from("Could not read plaintext temporary file."));
		};
		hash = format_to_hashmap(file_data);

		for key in &all_keys {
			let Some(val) = hash.get(key) else {
				continue;
			};
			if val.is_empty() {
				hash.remove(key);
			}
		}

		let mut mod_segment_header_vec = vec![String::from("--edit"), String::from("info")];
		for key in FROM_SEGMENT_HEADER {
			let (m, a) = match hash.get(*key) {
				None => (String::from("-d"), String::from(*key)),
				Some(val) => (String::from("-s"), format!("{}={}", key, val)),
			};
			mod_segment_header_vec.push(m);
			mod_segment_header_vec.push(a);
		}

		let mut had_metadata_child = false;
		for c in &mut xml.children {
			if let XMLNode::Element(child) = c {
				if !is_metadata_child(child) {
					continue;
				}
				let mut handled_tags: Vec<String> = vec![];
				let mut i = 0;
				while i < child.children.len() {
					if let Some(XMLNode::Element(tag)) = child.children.get_mut(i) {
						if tag.name != *"Simple" {
							i += 1;
							continue;
						}
						let Some(key) = tag.get_child("Name").map(|c| {
							c.get_text()
								.map(|s| s.into_owned())
								.unwrap_or(String::from(""))
						}) else {
							i += 1;
							continue;
						};
						if !FROM_TAGS.contains(&key.as_str()) {
							i += 1;
							continue;
						}
						handled_tags.push(key.clone());
						match hash.get(&key) {
							Some(val) => {
								let s = tag.get_mut_child("String").expect(
									"'Simple' tag in outputted xml didn't have 'String' child.",
								);
								s.children.clear();
								s.children.push(XMLNode::Text(val.clone()));
								i += 1;
							}
							None => {
								child.children.swap_remove(i);
							}
						}
					}
				}
				for tag in FROM_TAGS {
					if handled_tags.contains(&String::from(*tag)) {
						continue;
					}
					let Some(val) = hash.get(*tag) else {
						continue;
					};
					let mut node = Element::new("Simple");
					let mut name = Element::new("Name");
					name.children.push(XMLNode::Text(String::from(*tag)));
					node.children.push(XMLNode::Element(name));
					let mut string = Element::new("String");
					string.children.push(XMLNode::Text(val.clone()));
					node.children.push(XMLNode::Element(string));
					child.children.push(XMLNode::Element(node));
				}
				had_metadata_child = true;
				break;
			}
		}
		if !had_metadata_child {
			let mut child = Element::new("Tag");
			child
				.children
				.push(XMLNode::Element(Element::new("Targets")));
			for tag in FROM_TAGS {
				let Some(val) = hash.get(*tag) else {
					continue;
				};
				let mut node = Element::new("Simple");
				let mut name = Element::new("Name");
				name.children.push(XMLNode::Text(String::from(*tag)));
				node.children.push(XMLNode::Element(name));
				let mut string = Element::new("String");
				string.children.push(XMLNode::Text(val.clone()));
				node.children.push(XMLNode::Element(string));
				child.children.push(XMLNode::Element(node));
			}
			xml.children.insert(0, XMLNode::Element(child));
		}
		// put the written data at the front (seems to not like when the data is at the end)
		for i in 1..xml.children.len() {
			let Some(XMLNode::Element(child)) = xml.children.get(i) else {
				continue;
			};
			if !is_metadata_child(child) {
				continue;
			}
			xml.children.swap(0, i);
		}
		// remove written data child if it's empty
		if let Some(XMLNode::Element(child)) = xml.children.first() {
			if is_metadata_child(child) && child.get_child("Simple").is_none() {
				xml.children.swap_remove(0);
			}
		};

		xml.write(&xml_temp_file)
			.expect("Could not write back to xml temp file");

		let Ok(_) = run_cmd!(mkvpropedit $inp_file $[mod_segment_header_vec] --tags "global:$xml_temp_file_path")
		else {
			return Err(String::from("Could not modify the mkv file!"));
		};
	}

	Ok(())
}

// fn main() {
//     let Err(err) = _main() else {
//         return;
//     };
//     eprintln!("{}", err);
// }
