use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

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
	) -> Self {
		Self {
			pc_scena: Box::new(pc_scena),
			evo_scena: evo_scena.as_ref().to_owned(),
			pc_text: pc_text.as_ref().to_owned(),
			evo_text: evo_text.as_ref().to_owned(),
			is_en,
			scena: HashMap::new(),
			text: HashMap::new(),
		}
	}

	pub fn load_tl(&self, data: &str) -> Box<dyn Translator> {
		if self.is_en {
			Box::new(translate::Translate::load(data))
		} else {
			Box::new(translate::NullTranslator)
		}
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

pub fn insert_portraits(mut a: scena::ed7::Scena, b: &scena::ed7::Scena) -> scena::ed7::Scena {
	assert_eq!(a.functions.len(), b.functions.len());
	for (Code(a), Code(b)) in a.functions.iter_mut().zip(b.functions.iter()) {
		let mut i = 0;
		let mut j = 0;
		while i < a.len() && j < b.len() {
			let FlatInsn::Insn(Insn::TextTalk(a1, a2) | Insn::TextMessage(a1, a2)) = &a[i]
				else { i += 1; continue; };
			let FlatInsn::Insn(Insn::TextTalk(b1, b2) | Insn::TextMessage(b1, b2)) = &b[j]
				else { j += 1; continue; };

			assert_eq!(a1, b1);

			if a2.pages.len() < b2.pages.len() {
				let a1 = *a1;
				let a2 = a2.clone();
				a.remove(i); // TextTalk | TextMessage
				assert_eq!(a.remove(i), FlatInsn::Insn(Insn::TextWait()));
				do_insert(a, i, a1, a2);
			} else {
				if a[i] != b[j] {
					println!();
					println!("{:?}", &a[i]);
					println!("{:?}", &b[j]);
				}
				i += 1;
				j += 1;
			}
		}
	}
	a
}

fn do_insert(a: &mut Vec<FlatInsn>, i: usize, a1: CharId, a2: Text) {
	match &mut a[i] {
		FlatInsn::Unless(_, l) => {
			let l = *l;
			do_insert(a, i+1, a1, a2.clone());
			let j = a.iter().position(|i| i == &FlatInsn::Label(l)).unwrap();
			do_insert(a, j+1, a1, a2);
		}
		FlatInsn::Insn(Insn::TextTalk(b1, b2) | Insn::TextMessage(b1, b2)) => {
			assert_eq!(&a1, b1);
			b2.pages.splice(0..0, a2.pages);
		}
		i => panic!("{i:?}")
	}
}
