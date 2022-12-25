use std::collections::VecDeque;

use regex::Regex;
use themelios::text::{Text, TextSegment};
use themelios::scena::code::InsnArgMut as IAM;

use crate::visit::VisitMut;

pub trait Translator {
	fn comment(&mut self, s: &str);
	fn translate(&mut self, s: &str) -> String;
}

pub struct Dump {}
impl Translator for Dump {
	fn comment(&mut self, s: &str) {
		println!("\n## {s} {{{{{{1");
	}

	fn translate(&mut self, s: &str) -> String {
		println!();
		for l in s.split('\n') {
			println!("{l}");
		}
		for l in s.split('\n') {
			println!("\t{l}");
		}
		s.to_owned()
	}
}

pub struct Nil;
impl Translator for Nil {
	fn comment(&mut self, _: &str) {}

	fn translate(&mut self, s: &str) -> String {
		panic!("no translation expected! {s}");
	}
}

pub struct Translate(VecDeque<(String, String)>);
impl Translate {
	pub fn load(i: &str) -> Translate {
		let mut lines = Vec::<(String, String)>::new();
		#[derive(PartialEq)]
		enum State { None, Raw, Tl }
		let mut state = State::None;
		for line in i.lines() {
			let line = line.split_once("##").map_or_else(|| line, |a| a.0.trim_end_matches(' '));

			if state == State::Tl && !line.starts_with('\t') {
				state = State::None;
			}

			if line.is_empty() && state == State::None {
				continue
			}

			if let Some(line) = line.strip_prefix('\t') {
				assert!(state != State::None);
				if state == State::Tl {
					lines.last_mut().unwrap().1.push('\n');
				}
				lines.last_mut().unwrap().1.push_str(line);
				state = State::Tl;
			} else {
				if state == State::Raw {
					lines.last_mut().unwrap().0.push('\n');
				} else {
					lines.push((String::new(), String::new()));
				}
				lines.last_mut().unwrap().0.push_str(line);
				state = State::Raw;
			}
		}
		assert!(state != State::Raw);
		Translate(lines.into())
	}
}

impl Drop for Translate {
	fn drop(&mut self) {
		if !self.0.is_empty() {
			panic!("Not all was translated! {:?}", &self.0);
		}
	}
}

impl Translator for Translate {
	fn comment(&mut self, _: &str) {}

	fn translate(&mut self, s: &str) -> String {
		assert_eq!(Some(s), self.0.front().map(|a| a.0.as_str()));
		self.0.pop_front().unwrap().1
	}
}

pub fn translate<T: Clone + VisitMut>(tl: &mut impl Translator, a: &T) -> T {
	let mut a = a.clone();
	a.accept_mut(&mut |a| {
		match a {
			IAM::Text(a) => *a = translate_text(tl, a),
			IAM::TextTitle(a) if !a.is_empty() => *a = tl.translate(a),
			IAM::MenuItem(a) if !a.is_empty() => *a = tl.translate(a),
			_ => {}
		}
	});
	a
}

fn text2str(t: &Text) -> String {
	let mut s = String::new();
	for i in t.iter() {
		match i {
			TextSegment::String(v) => s.push_str(v),
			TextSegment::Line => s.push('\n'),
			TextSegment::Line2 => s.push('\r'),
			TextSegment::Wait => s.push_str("{wait}"),
			TextSegment::Page => s.push_str("{page}"),
			TextSegment::Color(v) => s.push_str(&format!("{{color {v}}}")),
			TextSegment::Item(v) => s.push_str(&format!("{{item {v}}}", v=v.0)),
			TextSegment::Byte(v) => s.push_str(&format!("{{#{v:02X}}}")),
		}
	}
	s
}

fn str2text(s: &str) -> Text {
	lazy_static::lazy_static! {
		static ref SEGMENT: Regex = Regex::new(r"(?x)
			(?P<t>.*?)
			(?:
				(?P<line>)\n|
				(?P<line2>)\r|
				(?P<wait>)\{wait\}|
				(?P<page>)\{page\}|
				\{color\ (?P<color>\d+)\}|
				\{item\ (?P<item>\d+)\}|
				\{\#(?P<hex>[[:xdigit:]]{2})\}|
				$
			)
		").unwrap();
	}
	let mut out = Text(Vec::new());
	for c in SEGMENT.captures_iter(s) {
		if let Some(t) = c.name("t") {
			if !t.as_str().is_empty() {
				out.push(TextSegment::String(t.as_str().to_owned()))
			}
		}
		if c.name("line").is_some() {
			out.push(TextSegment::Line)
		}
		if c.name("line2").is_some() {
			out.push(TextSegment::Line2)
		}
		if c.name("wait").is_some() {
			out.push(TextSegment::Wait)
		}
		if c.name("page").is_some() {
			out.push(TextSegment::Page)
		}
		if let Some(c) = c.name("color") {
			out.push(TextSegment::Color(c.as_str().parse().unwrap()))
		}
		if let Some(c) = c.name("item") {
			out.push(TextSegment::Item(c.as_str().parse::<u16>().unwrap().into()))
		}
		if let Some(c) = c.name("hex") {
			out.push(TextSegment::Byte(u8::from_str_radix(c.as_str(), 16).unwrap()))
		}
	}
	out
}

fn translate_text(tl: &mut impl Translator, t: &Text) -> Text {
	lazy_static::lazy_static! {
		static ref CONTENT: Regex = Regex::new(r"(?xs)
			^
			((?:\{.*?\}|\#\d+[S])*)
			((?:\#\d*[ABFNPVWZ])*)
			(.*?)
			((?:\{wait\})?)
			$
		").unwrap();
	}
	let s = text2str(t);
	assert_eq!(t, &str2text(&s));
	let s2 = s.split("{page}").map(|p| {
		let c = CONTENT.captures(p).unwrap();
		format!("{}{}{}", &c[2], tl.translate(&format!("{}{}", &c[1], &c[3]).replace('\r', "\n")), &c[4])
	}).collect::<Vec<_>>().join("{page}");
	str2text(&s2)
}
