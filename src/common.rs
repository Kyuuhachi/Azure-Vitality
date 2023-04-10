use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use regex::Regex;

use themelios::scena;
use themelios::scena::code::{Expr, Code, FlatInsn, Insn};
use themelios::scena::decompile::{TreeInsn, decompile, recompile};
use themelios::tables::quest;
use themelios::text::Text;
use themelios::types::*;

use crate::translate::{self, Translator, Translatable};
use crate::visit;

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

pub struct Context<'a> {
	pc_scena:   Box<dyn FnMut(&str) -> scena::ed7::Scena + 'a>,
	evo_scena:  PathBuf,
	pc_text:    PathBuf,
	evo_text:   PathBuf,

	pub is_en: bool,
	pub has_portraits: bool,

	pub scena: HashMap<String, AScena>,
	pub text: HashMap<String, Vec<u8>>,
}

pub fn load_scena(dir: impl AsRef<Path>, name: &str) -> anyhow::Result<scena::ed7::Scena> {
	let data = fs::read(dir.as_ref().join(format!("{name}.bin")))?;
	Ok(scena::ed7::read(Game::AoKai, &data).unwrap())
}

pub fn load_scena_evo(dir: impl AsRef<Path>, name: &str) -> anyhow::Result<scena::ed7::Scena> {
	let data = fs::read(dir.as_ref().join(format!("{name}.bin")))?;
	Ok(scena::ed7::read(Game::AoEvo, &data).unwrap())
}

impl<'a> Context<'a> {
	pub fn new(
		pc_scena: impl FnMut(&str) -> scena::ed7::Scena + 'a,
		evo_scena: impl AsRef<Path>,
		pc_text: impl AsRef<Path>,
		evo_text: impl AsRef<Path>,
		is_en: bool,
		has_portraits: bool,
	) -> Self {
		Self {
			pc_scena: Box::new(pc_scena),
			evo_scena: evo_scena.as_ref().to_owned(),
			pc_text: pc_text.as_ref().to_owned(),
			evo_text: evo_text.as_ref().to_owned(),
			is_en,
			has_portraits,
			scena: HashMap::new(),
			text: HashMap::new(),
		}
	}

	pub fn load_tl(&self, data: &str) -> translate::Translate {
		translate::Translate::load(data, self.is_en, self.has_portraits)
	}

	pub fn scena(&mut self, name: &str) -> &mut AScena {
		self.scena.entry(name.to_owned()).or_insert_with(|| {
			AScena {
				pc: (self.pc_scena)(name),
				evo: load_scena_evo(&self.evo_scena, name).unwrap(),
				new_npcs: Vec::new(),
				new_lps: Vec::new(),
				new_funcs: Vec::new(),
			}
		})
	}

	pub fn copy_scena(&mut self, name: &str, tl: &mut impl Translator) -> &mut AScena {
		self.scena.entry(name.to_owned()).or_insert_with(|| {
			let evo = load_scena_evo(&self.evo_scena, name).unwrap();
			let new_npcs = (0..evo.npcs.len()).collect();
			let new_lps = (0..evo.look_points.len()).collect();
			let new_funcs = (0..evo.functions.len()).collect();
			let mut pc = evo.clone();
			pc.labels.iter_mut().flatten().for_each(|a| a.name.translate(tl));
			pc.npcs.iter_mut().for_each(|a| a.name.translate(tl));
			pc.functions.iter_mut().for_each(|a| a.0.translate(tl));
			AScena {
				pc,
				evo,
				new_npcs,
				new_lps,
				new_funcs,
			}
		})
	}

	pub fn text(&mut self, name: &str) -> (&mut Vec<u8>, Vec<u8>) {
		let pc = self.text.entry(name.to_owned()).or_insert_with(|| {
			fs::read(self.pc_text.join(name)).unwrap()
		});
		let evo = fs::read(self.evo_text.join(name)).unwrap();
		(pc, evo)
	}

	pub fn copy_quest(&mut self, id: QuestId, tl: &mut impl Translator) {
		let (pc, evo) = self.text("t_quest._dt");
		let mut pc_quests = quest::read_ed7(pc).unwrap();
		let evo_quests = quest::read_ed7(&evo).unwrap();
		let mut q = evo_quests.iter().find(|a| a.id == id).unwrap().clone();
		q.name.translate(tl);
		q.client.translate(tl);
		q.desc.translate(tl);
		q.steps.translate(tl);
		*pc_quests.iter_mut().find(|a| a.id == id).unwrap() = q;
		*pc = quest::write_ed7(&pc_quests).unwrap();
	}
}

pub fn copy_shape(scena: &mut scena::ed7::Scena, shape: &scena::ed7::Scena) {
	for (i, (a, b)) in scena.functions.iter_mut().zip(shape.functions.iter()).enumerate() {
		let mut extract = translate::Extract::new();
		a.0.translate(&mut extract);
		let mut inject = translate::Inject::new(extract.finish());
		let mut c = b.clone();
		c.0.translate(&mut inject);
		if inject.finish() {
			*a = c;
		} else {
			eprintln!("failed to copy shape of fn[{i}]");
		}
	}
}

pub struct AScena {
	pub pc: scena::ed7::Scena,
	pub evo: scena::ed7::Scena,

	pub new_npcs: Vec<usize>,
	pub new_lps: Vec<usize>,
	pub new_funcs: Vec<usize>,
}

impl AScena {
	pub fn copy_npc(&mut self, idx: usize, tl: &mut impl Translator) {
		let monster_start = (8+self.pc.npcs.len()) as u16;
		let monster_end = (8+self.pc.npcs.len()+self.pc.monsters.len()) as u16;
		visit::char_id::ed7scena(&mut self.pc, &mut |a| {
			if a.0 >= monster_start && a.0 < monster_end {
				a.0 += 1;
			}
		});

		let new_idx = self.pc.npcs.len();

		let start = 8 + idx as u16;
		let end = 8 + new_idx as u16;
		visit::char_id::ed7scena(&mut self.evo, &mut |a| {
			if a.0 == start {
				a.0 = end;
			} else if start < a.0 && a.0 <= end {
				a.0 -= 1;
			}
		});

		let npc = self.evo.npcs.remove(idx);
		let mut npc2 = npc.clone();
		npc2.name.translate(tl);
		self.pc.npcs.insert(new_idx, npc2);
		self.evo.npcs.insert(new_idx, npc);
		self.new_npcs.push(new_idx);
	}

	pub fn copy_func(&mut self, scp: u16, idx: usize, tl: &mut impl Translator) {
		let new_idx = self.pc.functions.len();

		let start = idx as u16;
		let end = new_idx as u16;
		let mut f = |a: &mut FuncId| {
			if a.0 == scp {
				if a.1 == start {
					a.1 = end;
				} else if start < a.1 && a.1 <= end {
					a.1 -= 1;
				}
			}
		};
		visit::func_id::ed7scena(&mut self.evo, &mut f);
		for &i in &self.new_npcs {
			f(&mut self.pc.npcs[i].init);
			f(&mut self.pc.npcs[i].talk);
		}
		for &i in &self.new_lps {
			f(&mut self.pc.look_points[i].function);
		}
		for &i in &self.new_funcs {
			visit::func_id::func(&mut self.pc.functions[i], &mut f);
		}

		let func = self.evo.functions.remove(idx);
		self.pc.functions.push(Code(func.0.translated(tl)));
		self.evo.functions.insert(new_idx, func);
		self.new_funcs.push(new_idx);
	}

	pub fn pad_func(&mut self, scp: u16, n: usize) {
		self.evo.functions.insert(n, Code(vec![]));
		visit::func_id::ed7scena(&mut self.evo, &mut |a| {
			if a.0 == scp && a.1 as usize >= n {
				a.1 += 1
			}
		});
	}

	pub fn pad_npc(&mut self, n: usize) {
		self.evo.npcs.insert(n, scena::ed7::Npc {
			name: "".into(),
			pos: Pos3(0,0,0),
			angle: Angle(0),
			flags: CharFlags(0),
			unk2: 0,
			chip: ChipId(0),
			init: FuncId(0xFF,0xFFFF),
			talk: FuncId(0xFF,0xFFFF),
			unk4: 0,
		});
		visit::char_id::ed7scena(&mut self.evo, &mut |a| {
			if a.0 as usize >= n + 8 && a.0 <= 200 {
				a.0 += 1
			}
		});
	}

	pub fn copy_look_point(&mut self, idx: usize) {
		let new_idx = self.pc.look_points.len();

		let start = idx as u16;
		let end = new_idx as u16;
		visit::look_point::ed7scena(&mut self.evo, &mut |a| {
			if a.0 == start {
				a.0 = end;
			} else if start < a.0 && a.0 <= end {
				a.0 -= 1;
			}
		});

		let lp = self.evo.look_points.remove(idx);
		self.pc.look_points.push(lp.clone());
		self.evo.look_points.insert(new_idx, lp);
		self.new_lps.push(new_idx);
	}

	pub fn func(&mut self, idx: usize, f: impl FnOnce(AList<Vec<TreeInsn>>)) {
		let mut f1 = decompile(&self.pc.functions[idx]).unwrap();
		let f2 = decompile(&self.evo.functions[idx]).unwrap();
		f(AList(&mut f1, &f2));
		self.pc.functions[idx] = recompile(&f1).unwrap();
	}
}

#[derive(Debug)]
pub struct AList<'a, T>(pub &'a mut T, pub &'a T);

pub macro alist_map($e:expr; $($t:tt)*) {
	{
		let x = $e;
		$crate::common::AList(
			x.0.iter_mut() $($t)*,
			x.1.iter() $($t)*,
		)
	}
}

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
	pub fn copy_clause(&mut self, k: &A) where (A, B): Clone + Translatable {
		self.0.push(self.1.iter().find(|a| &a.0 == k).unwrap().no_tl());
	}
}

impl<'a, T> AList<'a, Vec<T>> {
	#[track_caller]
	pub fn copy_tail(&mut self) where T: Clone + Translatable {
		self.0.extend(self.1[self.0.len()..].iter().map(|a| a.no_tl()))
	}

	#[track_caller]
	pub fn index_of(&self, f: impl Fn(&T) -> bool) -> (usize, usize) {
		(
			self.0.iter().enumerate().find_map(|(a, b)| f(b).then_some(a)).unwrap(),
			self.1.iter().enumerate().find_map(|(a, b)| f(b).then_some(a)).unwrap(),
		)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit<A, B> {
	Del(A),
	Eq(A, B),
	Ins(B),
}

#[allow(clippy::erasing_op, clippy::identity_op)]
fn align<'a, 'b, A: std::fmt::Debug, B: std::fmt::Debug>(
	a: &'a [A],
	b: &'b [B],
	mut score: impl FnMut(Edit<&'a A, &'b B>) -> i32,
) -> Vec<Edit<&'a A, &'b B>> {
	let stride = a.len()+1;
	let mut edits = Vec::with_capacity((a.len()+1)*(b.len()+1));
	let mut scores = Vec::with_capacity((a.len()+1)*(b.len()+1));
	scores.push(0);
	for (i, a) in a.iter().rev().enumerate() {
		let i = i+1;
		let j = 0;
		let e = (scores[(i-1)+(j-0)*stride], Edit::Del(a));
		scores.push(e.0 + score(e.1));
		edits.push(e.1);
	}

	for (j, b) in b.iter().rev().enumerate() {
		let i = 0;
		let j = j+1;
		let e = (scores[(i-0)+(j-1)*stride], Edit::Ins(b));
		scores.push(e.0 + score(e.1));
		edits.push(e.1);

		for (i, a) in a.iter().rev().enumerate() {
			let i = i+1;
			let e = [
				(scores[(i-1)+(j-0)*stride], Edit::Del(a)),
				(scores[(i-1)+(j-1)*stride], Edit::Eq(a, b)),
				(scores[(i-0)+(j-1)*stride], Edit::Ins(b))
			].into_iter().max_by_key(|e| e.0 + score(e.1)).unwrap();
			scores.push(e.0 + score(e.1));
			edits.push(e.1);
		}
	}

	let mut out = Vec::new();
	let mut p = edits.len();
	while p > 0 {
		out.push(edits[p-1]);
		p -= match edits[p-1] {
			Edit::Del(_) => 1,
			Edit::Eq(_, _) => stride + 1,
			Edit::Ins(_) => stride,
		}
	}
	out
}

pub fn insert_portraits(a: &scena::ed7::Scena, b: &scena::ed7::Scena) -> scena::ed7::Scena {
	let mut a = a.clone();
	assert_eq!(a.functions.len(), b.functions.len());
	for (Code(a), Code(b)) in a.functions.iter_mut().zip(b.functions.iter()) {
		use FlatInsn as FI;
		let align = align(a, b, insn_alignment_score);

		let mut c = Vec::new();
		for e in align {
			match e {
				Edit::Del(FI::Goto(l)) => c.push(FI::Goto(*l)),
				Edit::Del(FI::Label(l)) => c.push(FI::Label(*l)), // XXX this messes up the label order.
				Edit::Del(_) => {},
				Edit::Eq(_, b) => c.push(b.clone()),
				Edit::Ins(b) => c.push(b.clone()),
			}
		}
		*a = c;
	}
	a
}

fn insn_alignment_score(e: Edit<&FlatInsn, &FlatInsn>) -> i32 {
	use FlatInsn as FI;
	match e {
		Edit::Del(a) => match a {
			FI::Goto(_) => -1, // TODO goto only if previous was also goto/return? Probably unnecessary
			FI::Label(_) => 0,
			_ => -5,
		},
		Edit::Eq(a, b) => match (a, b) {
			(FI::Unless(a, _), FI::Unless(b, _)) => if a == b { 10 } else { 1 },
			(FI::Goto(_), FI::Goto(_)) => 5,
			(FI::Switch(a, _, _), FI::Switch(b, _, _)) => if a == b { 10 } else { 1 },
			(FI::Insn(a), FI::Insn(b)) => {
				if a == b {
					10
				} else if std::mem::discriminant(a) == std::mem::discriminant(b) {
					1
				} else {
					-10
				}
			},
			(FI::Label(_), FI::Label(_)) => 5,
			_ => -100, // matching control flow in mismatched ways is almost never right
		},
		Edit::Ins(_) => -5,
	}
}
