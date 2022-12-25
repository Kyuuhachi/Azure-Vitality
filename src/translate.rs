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
		println!("## {s}");
	}

	fn translate(&mut self, s: &str) -> String {
		println!("{s:?}");
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
			((?:\#\d*[ABFPVWZ])*)
			(.*?)
			((?:\{wait\})?)
			$
		").unwrap();
	}
	let s = text2str(t);
	assert_eq!(t, &str2text(&s));
	let s2 = s.split("{page}").map(|p| {
		let c = CONTENT.captures(p).unwrap();
		format!("{}{}{}", &c[2], tl.translate(&format!("{}{}", &c[1], &c[3])), &c[4])
	}).collect::<Vec<_>>().join("{page}");
	str2text(&s2)
}
