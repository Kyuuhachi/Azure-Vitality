use std::path::{Path, PathBuf};
use std::collections::HashMap;

use themelios::gamedata::GameData;
use themelios::scena;
use themelios::scena::code::{Expr, FlatInsn, Insn, InsnArgMut as IAM};
use themelios::scena::code::decompile::{TreeInsn, decompile, recompile};
use themelios::scena::ed7::Scena;
use themelios::tables::quest::{ED7Quest, self};
use themelios::text::Text;
use themelios::types::{QuestId, Flag};
use visit::VisitMut;

mod visit;
mod translate;
use translate::{Nil, Dump, Translator, translate};

macro_rules! f {
	($p:pat => $v:expr) => { |_a| {
		#[allow(irrefutable_let_patterns)]
		if let $p = _a { Some($v) } else { None }
	} };
	($p:pat) => { |_a| {
		matches!(_a, $p)
	} };
}

struct Context {
	psp_path:   PathBuf,
	evo_path:   PathBuf,
	gf_path:    PathBuf,
	scenas:     HashMap<String, AScena>,
	quests:     Vec<ED7Quest>,
	evo_quests: Vec<ED7Quest>,
}

impl Context {
	fn new(
		psp: impl AsRef<Path>,
		evo: impl AsRef<Path>,
		gf: impl AsRef<Path>,
		quests: impl AsRef<Path>,
		evo_quests: impl AsRef<Path>,
	) -> Context {
		Context {
			psp_path: psp.as_ref().to_owned(),
			evo_path: evo.as_ref().to_owned(),
			gf_path: gf.as_ref().to_owned(),
			scenas: HashMap::new(),
			quests: quest::read_ed7(GameData::AO, &std::fs::read(quests).unwrap()).unwrap(),
			evo_quests: quest::read_ed7(GameData::AO_EVO, &std::fs::read(evo_quests).unwrap()).unwrap(),
		}
	}

	fn scena(&mut self, name: &str) -> &mut AScena {
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

	fn copy_scena(&mut self, name: &str, tl: &mut impl Translator) -> &mut AScena {
		todo!("copy a whole scena, usually a _1, wholesale")
	}

	fn copy_quest(&mut self, id: QuestId, tl: &mut impl Translator) -> &mut ED7Quest {
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

struct AScena {
	main: Scena,
	evo: Scena,

	new_npcs: Vec<usize>,
	new_lps: Vec<usize>,
	new_funcs: Vec<usize>,
}

impl AScena {
	fn remap(&mut self, v: &mut impl FnMut(IAM)) {
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

	fn copy_npc(&mut self, idx: usize, tl: &mut impl Translator) {
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

	fn copy_func(&mut self, scp: u16, idx: usize, tl: &mut impl Translator) {
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
		self.main.functions.insert(new_idx, translate(tl, &func));
		self.evo.functions.insert(new_idx, func);
		self.new_funcs.push(new_idx);
	}

	fn copy_look_point(&mut self, idx: usize) -> usize {
		todo!();
	}

	fn func(&mut self, idx: usize, f: impl FnOnce(&mut AList<Vec<TreeInsn>>)) {
		let mut f1 = decompile(&self.main.functions[idx]).unwrap();
		let f2 = decompile(&self.evo.functions[idx]).unwrap();
		f(&mut AList { main: &mut f1, evo: &f2 });
		self.main.functions[idx] = recompile(&f1).unwrap();
	}
}

struct AList<'a, T> {
	main: &'a mut T,
	evo: &'a T,
}

impl AList<'_, Vec<TreeInsn>> {
	#[track_caller]
	fn ifs(&mut self, n: usize) -> AList<Vec<(Option<Expr>, Vec<TreeInsn>)>> {
		AList {
			main: self.main.iter_mut().filter_map(f!(TreeInsn::If(x) => x)).nth(n).unwrap(),
			evo:  self.evo     .iter().filter_map(f!(TreeInsn::If(x) => x)).nth(n).unwrap(),
		}
	}
}

impl<A: PartialEq, B> AList<'_, Vec<(A, B)>> {
	#[track_caller]
	fn clause(&mut self, k: &A) -> AList<B> {
		AList {
			main: self.main.iter_mut().find_map(|(a,b)| (a == k).then_some(b)).unwrap(),
			evo:  self.evo     .iter().find_map(|(a,b)| (a == k).then_some(b)).unwrap(),
		}
	}
}

impl<T: Clone + VisitMut> AList<'_, Vec<T>> {
	#[track_caller]
	fn tail(&mut self, tl: &mut impl Translator) {
		self.main.extend(self.evo[self.main.len()..].iter().map(|a| translate(tl, a)))
	}
}

macro_rules! flag {
	($n:literal) => { Expr::Flag(Flag($n)) }
}

fn main() {
	let mut ctx = Context::new(
		"./data/ao-psp/PSP_GAME/USRDIR/data/scena/",
		"./data/vita/extract/ao/data1/data/scena/",
		"./data/ao-gf/data_en/scena/",
		"./data/ao-gf/data_en/text/t_quest._dt",
		"./data/vita/extract/ao/data/data/text/t_quest._dt",
	);
	quest125(&mut ctx);
}

fn quest125(ctx: &mut Context) {
	let nil = &mut Nil;

	let tl = &mut Dump {};
	ctx.copy_quest(QuestId(125), tl);

	tl.comment("c1300 - IBC exterior");
	let s = ctx.scena("c1300");
	s.main.chcp.push(Some("chr/ch06000.itc".to_owned()));
	s.copy_npc(1, tl);  // Grace
	s.copy_npc(10, tl); // Shirley
	s.copy_npc(11, tl); // Sigmund
	s.copy_func(0, 9, tl); // talk Grace
	s.func(1, |a| a.ifs(1).clause(&Some(flag![2564])).tail(nil));

	tl.comment("c1200 - Harbor District");
	let s = ctx.scena("c1200");
	s.main.chcp[19] = Some("chr/ch28100.itc".to_owned());
	s.copy_npc(31, tl); // Reins
	s.copy_func(0, 107, tl); // talk Reins

	tl.comment("c0490 - Neue Blanc");
	let s = ctx.scena("c0490");
	s.copy_npc(18, tl); // Wazy's patron
	s.copy_npc(19, tl); // Grace
	s.copy_npc(20, tl); // Woman
	s.copy_npc(21, tl); // Man
	s.copy_npc(22, tl); // Man
	s.copy_npc(23, tl); // Imperial mafioso
	s.copy_npc(24, tl); // Republic mafioso
	s.copy_func(0, 15, tl); // main event
	s.copy_func(0, 16, tl); // fork
	s.copy_func(0, 17, tl); // camera preset
	s.copy_func(0, 18, tl); // camera preset
	s.copy_func(0, 19, tl); // fork
	s.copy_func(0, 20, tl); // fork
	s.copy_func(0, 21, tl); // fork
	s.copy_func(0, 22, tl); // fork: shake
	s.copy_func(0, 23, tl); // fork: emote
	s.func(1, |a| a.ifs(0).tail(nil));
}
