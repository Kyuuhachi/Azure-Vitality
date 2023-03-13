use std::collections::VecDeque;

use regex::Regex;
use themelios::scena::code::{FlatInsn, Insn, Expr};
use themelios::text::{Text, TextSegment};
use themelios::scena::decompile::TreeInsn;
use themelios::types::TString;

pub trait Translator {
	fn text(&mut self, s: &mut Text);
	fn tstring(&mut self, s: &mut TString);
}

pub struct NullTranslator;

impl Translator for NullTranslator {
	fn text(&mut self, _: &mut Text) {}
	fn tstring(&mut self, _: &mut TString) {}
}

// #[deprecated]
// pub struct Dump;
// #[allow(deprecated)]
// impl Translator for Dump {
// 	fn translate(&mut self, s: &str) -> String {
// 		println!();
// 		for l in s.split('\n') {
// 			println!("{l}");
// 		}
// 		for l in s.split('\n') {
// 			println!("\t{l}");
// 		}
// 		s.to_owned()
// 	}
// }

pub struct Nil;
impl Translator for Nil {
	fn text(&mut self, s: &mut Text) {
		panic!("no translation expected! {}", text2str(s));
	}
	fn tstring(&mut self, s: &mut TString) {
		panic!("no translation expected! {}", &s.0);
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
				let (a, b) = lines.last().unwrap();
				if a == b {
					println!("{:?}", a);
				}
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

	fn translate(&mut self, s: &str) -> String {
		if s.is_empty() {
			return String::new();
		}

		// println!("{:?}", s);
		// let f = std::backtrace::Backtrace::force_capture();
		// let f = f.frames();
		// let f = &f[3..f.len()-10];
		// for f in f {
		// 	println!("{:?}", f);
		// }
		// println!();

		let a = self.0.front().map(|a| a.0.as_str());
		if Some(s) != a {
			println!("{:?}\n{:?}\n", Some(s), a);
		}

		self.0.pop_front().unwrap().1
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
	fn text(&mut self, s: &mut Text) {
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
		let ss = text2str(s);
		assert_eq!(s, &str2text(&ss));
		let s2 = ss.split("{page}").map(|p| {
			let c = CONTENT.captures(p).unwrap();
			let t = self.translate(&format!("{}{}", &c[1], &c[3]).replace('\r', "\n"));
			format!("{}{}{}", &c[2], t, &c[4])
		}).collect::<Vec<_>>().join("{page}");
		*s = str2text(&s2);
	}

	fn tstring(&mut self, s: &mut TString) {
		s.0 = self.translate(&s.0);
	}
}

pub enum TlObj {
	Text(Text),
	TString(TString),
}

pub struct Extract {
	strings: Vec<TlObj>
}

impl Extract {
	pub fn new() -> Self {
		Self {
			strings: Vec::new(),
		}
	}

	pub fn finish(self) -> Vec<TlObj> {
		self.strings
	}
}

impl Translator for Extract {
	fn text(&mut self, s: &mut Text) {
		self.strings.push(TlObj::Text(s.clone()));
	}

	fn tstring(&mut self, s: &mut TString) {
		self.strings.push(TlObj::TString(s.clone()));
	}
}

pub struct Inject {
	strings: VecDeque<TlObj>,
	failed: bool,
}

impl Inject {
	pub fn new(strings: Vec<TlObj>) -> Self {
		Self {
			strings: strings.into(),
			failed: false,
		}
	}

	pub fn finish(&self) -> bool {
		self.strings.is_empty() && !self.failed
	}
}

impl Translator for Inject {
	fn text(&mut self, s: &mut Text) {
		match self.strings.pop_front() {
			Some(TlObj::Text(z)) => *s = z,
			_ => self.failed = true,
		}
	}

	fn tstring(&mut self, s: &mut TString) {
		match self.strings.pop_front() {
			Some(TlObj::TString(z)) => *s = z,
			_ => self.failed = true,
		}
	}
}

impl<T: Translator + ?Sized> Translator for Box<T> {
	fn text(&mut self, s: &mut Text) {
		Box::as_mut(self).text(s)
	}

	fn tstring(&mut self, s: &mut TString) {
		Box::as_mut(self).tstring(s)
	}
}

pub trait Translatable {
	fn translate(&mut self, tl: &mut impl Translator);

	fn translated(&self, tl: &mut impl Translator) -> Self where Self: Clone {
		let mut a = self.clone();
		a.translate(tl);
		a
	}

	fn no_tl(&self) -> Self where Self: Clone {
		let mut a = self.clone();
		a.translate(&mut Nil);
		a
	}
}

impl Translatable for Text {
	fn translate(&mut self, tl: &mut impl Translator) {
		tl.text(self);
	}
}

impl Translatable for TString {
	fn translate(&mut self, tl: &mut impl Translator) {
		tl.tstring(self);
	}
}

impl<T: Translatable> Translatable for Vec<T> {
	fn translate(&mut self, tl: &mut impl Translator) {
		self.iter_mut().for_each(|a| a.translate(tl))
	}
}

impl<T: Translatable> Translatable for Option<T> {
	fn translate(&mut self, tl: &mut impl Translator) {
		self.iter_mut().for_each(|a| a.translate(tl))
	}
}

impl<T: Translatable, U: Translatable> Translatable for (T, U) {
	fn translate(&mut self, tl: &mut impl Translator) {
		self.0.translate(tl);
		self.1.translate(tl);
	}
}

impl Translatable for FlatInsn {
	fn translate(&mut self, tl: &mut impl Translator) {
		if let Self::Insn(a) = self { a.translate(tl) }
	}
}

impl Translatable for TreeInsn {
	fn translate(&mut self, tl: &mut impl Translator) {
		if let Self::Insn(a) = self { a.translate(tl) }
	}
}

impl Translatable for Insn {
	fn translate(&mut self, tl: &mut impl Translator) {
		macro run {
			([$(($ident:ident $(($_n:ident $($ty:tt)*))*))*]) => {
				match self {
					$(Insn::$ident($($_n),*) => {
						$(run!($_n $($ty)*);)*
					})*
				}
			},
			($v:ident Text) => { tl.text($v); },
			($v:ident TString) => { tl.tstring($v); },
			($v:ident Vec<TString>) => { for i in $v { tl.tstring(i) } },
			($i:ident $($t:tt)*) => {}
		}
		themelios::scena::code::introspect!(run);
	}
}

impl Translatable for Expr {
	fn translate(&mut self, _: &mut impl Translator) {
		// There aren't any translatable strings in exprs
	}
}

pub fn text2str(t: &Text) -> String {
	let mut s = String::new();
	for i in t.iter() {
		match i {
			TextSegment::String(v) => s.push_str(v),
			TextSegment::Line => s.push('\n'),
			TextSegment::Wait => s.push_str("{wait}"),
			TextSegment::Page => s.push_str("{page}"),
			TextSegment::Color(v) => s.push_str(&format!("{{color {v}}}")),
			TextSegment::Item(v) => s.push_str(&format!("{{item {v}}}", v=v.0)),
			TextSegment::Byte(v) => s.push_str(&format!("{{#{v:02X}}}")),
		}
	}
	s
}

pub fn str2text(s: &str) -> Text {
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
