use std::path::{Path, PathBuf};
use std::collections::HashMap;

use themelios::gamedata::GameData;
use themelios::scena;
use themelios::scena::code::{Expr, FlatInsn, InsnArgMut as IAM};
use themelios::scena::code::decompile::{TreeInsn, decompile, recompile};
use themelios::scena::ed7::Scena;
use themelios::tables::quest::ED7Quest;
use themelios::text::Text;
use themelios::types::QuestId;

use crate::visit::VisitMut;
use crate::translate::{Translator, translate};

// Hack for syntax highlighting not supporting macros 2.0
mod m {
	pub macro f {
		($p:pat $(if $e:expr)? => $v:expr) => { |_a| {
			match _a {
				$p $(if $e)? => Some($v),
				_ => None
			}
		} },
		($p:pat $(if $e:expr)? ) => { |_a| {
			match _a {
				$p $(if $e)? => true,
				_ => false
			}
		} },
	}
}
pub use m::f;

pub struct Context {
	psp_path:   PathBuf,
	evo_path:   PathBuf,
	gf_path:    PathBuf,
	pub scenas: HashMap<String, AScena>,
	pub quests: Vec<ED7Quest>,
	evo_quests: Vec<ED7Quest>,
}

impl Context {
	pub fn new(
		psp: impl AsRef<Path>,
		evo: impl AsRef<Path>,
		gf: impl AsRef<Path>,
		quests: Vec<ED7Quest>,
		evo_quests: Vec<ED7Quest>,
	) -> Context {
		Context {
			psp_path: psp.as_ref().to_owned(),
			evo_path: evo.as_ref().to_owned(),
			gf_path: gf.as_ref().to_owned(),
			scenas: HashMap::new(),
			quests,
			evo_quests,
		}
	}

	pub fn scena(&mut self, name: &str) -> &mut AScena {
		self.scenas.entry(name.to_owned()).or_insert_with(|| {
			let mut gf = scena::ed7::read(GameData::AO, &std::fs::read(self.gf_path.join(format!("{name}.bin"))).unwrap()).unwrap();
			let psp = scena::ed7::read(GameData::AO, &std::fs::read(self.psp_path.join(format!("{name}.bin"))).unwrap()).unwrap();
			let evo = scena::ed7::read(GameData::AO_EVO, &std::fs::read(self.evo_path.join(format!("{name}.bin"))).unwrap()).unwrap();
			assert_eq!(gf.functions.len(), psp.functions.len());
			for (i, (a, b)) in gf.functions.iter_mut().zip(psp.functions.iter()).enumerate() {
				if let Some(c) = merge_gf(a, b) {
					*a = c;
				} else {
					eprintln!("failed to merge {name}:{i}, using plain GF");
				}
			}
			AScena {
				main: gf,
				evo,
				new_npcs: Vec::new(),
				new_lps: Vec::new(),
				new_funcs: Vec::new(),
			}
		})
	}

	pub fn copy_scena(&mut self, name: &str, tl: &mut impl Translator) -> &mut AScena {
		self.scenas.entry(name.to_owned()).or_insert_with(|| {
			let evo = scena::ed7::read(GameData::AO_EVO, &std::fs::read(self.evo_path.join(format!("{name}.bin"))).unwrap()).unwrap();
			let new_npcs = (0..evo.npcs.len()).collect();
			let new_lps = (0..evo.look_points.len()).collect();
			let new_funcs = (0..evo.functions.len()).collect();
			AScena {
				main: translate(tl, &evo),
				evo,
				new_npcs,
				new_lps,
				new_funcs,
			}
		})
	}

	pub fn copy_quest(&mut self, id: QuestId, tl: &mut impl Translator) -> &mut ED7Quest {
		let q1 = self.quests.iter_mut().find(|a| a.id == id).unwrap();
		let q2 = self.evo_quests.iter().find(|a| a.id == id).unwrap();
		*q1 = translate(tl, q2);
		q1
	}
}

fn merge_gf(gf: &[FlatInsn], psp: &[FlatInsn]) -> Option<Vec<FlatInsn>> {
	let mut gf = gf.to_owned();
	let mut psp = psp.to_owned(); // Because I don't have any non-mut visit
	enum I {
		Text(Text),
		TextTitle(String),
		MenuItem(String),
	}
	let mut texts = Vec::new();
	gf.accept_mut(&mut |a| match a {
		IAM::Text(a) => texts.push(I::Text(a.clone())),
		IAM::TextTitle(a) => texts.push(I::TextTitle(a.clone())),
		IAM::MenuItem(a) => texts.push(I::MenuItem(a.clone())),
		_ => {}
	});
	texts.reverse();
	let mut success = true;
	psp.accept_mut(&mut |a| match a {
		IAM::Text(a) => {
			if let Some(I::Text(b)) = texts.pop() {
				*a = b
			} else {
				success = false
			}
		}
		IAM::TextTitle(a) => {
			if let Some(I::TextTitle(b)) = texts.pop() {
				*a = b
			} else {
				success = false
			}
		}
		IAM::MenuItem(a) => {
			if let Some(I::MenuItem(b)) = texts.pop() {
				*a = b
			} else {
				success = false
			}
		}
		_ => {}
	});
	success &= texts.is_empty();
	success.then_some(psp)
}

pub struct AScena {
	pub main: Scena,
	pub evo: Scena,

	pub new_npcs: Vec<usize>,
	pub new_lps: Vec<usize>,
	pub new_funcs: Vec<usize>,
}

impl AScena {
	pub fn remap(&mut self, v: &mut impl FnMut(IAM)) {
		self.evo.accept_mut(v);
		for i in &self.new_npcs {
			self.main.npcs[*i].accept_mut(v);
		}
		for i in &self.new_lps {
			self.main.look_points[*i].accept_mut(v);
		}
		for i in &self.new_funcs {
			self.main.functions[*i].accept_mut(v);
		}
	}

	pub fn copy_npc(&mut self, idx: usize, tl: &mut impl Translator) {
		let monster_start = (8+self.main.npcs.len()) as u16;
		let monster_end = (8+self.main.npcs.len()+self.main.monsters.len()) as u16;
		self.main.accept_mut(&mut |a| {
			if let IAM::CharId(a) = a {
				if a.0 >= monster_start && a.0 < monster_end {
					a.0 += 1;
				}
			}
		});

		let new_idx = self.main.npcs.len();

		let start = 8 + idx as u16;
		let end = 8 + new_idx as u16;
		self.remap(&mut |a| {
			if let IAM::CharId(a) = a {
				if a.0 == start {
					a.0 = end;
				} else if start < a.0 && a.0 <= end {
					a.0 -= 1;
				}
			}
		});

		let npc = self.evo.npcs.remove(idx);
		self.main.npcs.insert(new_idx, translate(tl, &npc));
		self.evo.npcs.insert(new_idx, npc);
		self.new_npcs.push(new_idx);
	}

	pub fn copy_func(&mut self, scp: u16, idx: usize, tl: &mut impl Translator) {
		let new_idx = self.main.functions.len();

		let start = idx as u16;
		let end = new_idx as u16;
		self.remap(&mut |a| {
			if let IAM::FuncRef(a) = a {
				if a.0 == scp {
					if a.1 == start {
						a.1 = end;
					} else if start < a.1 && a.1 <= end {
						a.1 -= 1;
					}
				}
			}
		});

		let func = self.evo.functions.remove(idx);
		self.main.functions.push(translate(tl, &func));
		self.evo.functions.insert(new_idx, func);
		self.new_funcs.push(new_idx);
	}

	pub fn copy_look_point(&mut self, idx: usize) {
		let new_idx = self.main.look_points.len();

		let start = idx as u16;
		let end = new_idx as u16;
		self.remap(&mut |a| {
			if let IAM::LookPointId(a) = a {
				if *a == start {
					*a = end;
				} else if start < *a && *a <= end {
					*a -= 1;
				}
			}
		});

		let lp = self.evo.look_points.remove(idx);
		self.main.look_points.push(lp.clone());
		self.evo.look_points.insert(new_idx, lp);
		self.new_lps.push(new_idx);
	}

	pub fn func(&mut self, idx: usize, f: impl FnOnce(AList<Vec<TreeInsn>>)) {
		let mut f1 = decompile(&self.main.functions[idx]).unwrap();
		let f2 = decompile(&self.evo.functions[idx]).unwrap();
		f(AList(&mut f1, &f2));
		self.main.functions[idx] = recompile(&f1).unwrap();
	}
}

#[derive(Debug)]
pub struct AList<'a, T>(pub &'a mut T, pub &'a T);

mod m2 {
	pub macro alist_map($e:expr; $($t:tt)*) {
		{
			let x = $e;
			$crate::common::AList(
				x.0.iter_mut() $($t)*,
				x.1.iter() $($t)*,
			)
		}
	}
}
pub use m2::alist_map;

impl<'a> AList<'a, Vec<TreeInsn>> {
	#[track_caller]
	pub fn if_with(self, e: &Expr) -> AList<'a, Vec<(Option<Expr>, Vec<TreeInsn>)>> {
		alist_map!(self; .find_map(f!(TreeInsn::If(x) if x.iter().any(|a| a.0.as_ref() == Some(e)) => x)).unwrap())
	}

	#[track_caller]
	pub fn if_clause(self, e: &Expr) -> AList<'a, Vec<TreeInsn>> {
		self.if_with(e).clause(&Some(e.clone()))
	}
}

impl<'a, A: PartialEq, B> AList<'a, Vec<(A, B)>> {
	#[track_caller]
	pub fn clause(self, k: &A) -> AList<'a, B> {
		alist_map!(self; .find_map(|(a,b)| (a == k).then_some(b)).unwrap())
	}

	#[track_caller]
	pub fn copy_clause(&mut self, k: &A, tl: &mut impl Translator) where (A, B): Clone + VisitMut {
		self.0.push(translate(tl, self.1.iter().find(|a| &a.0 == k).unwrap()));
	}
}

impl<'a, T> AList<'a, Vec<T>> {
	#[track_caller]
	pub fn copy_tail(&mut self, tl: &mut impl Translator) where T: Clone + VisitMut {
		self.0.extend(self.1[self.0.len()..].iter().map(|a| translate(tl, a)))
	}

	#[track_caller]
	pub fn index_of(&self, f: impl Fn(&T) -> bool) -> (usize, usize) where T: Clone + VisitMut {
		(
			self.0.iter().enumerate().find_map(|(a, b)| f(b).then_some(a)).unwrap(),
			self.1.iter().enumerate().find_map(|(a, b)| f(b).then_some(a)).unwrap(),
		)
	}
}
