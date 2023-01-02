use std::path::{Path, PathBuf};
use std::collections::HashMap;

use themelios::gamedata::GameData;
use themelios::scena::{self, FuncRef};
use themelios::scena::code::{Expr, FlatInsn, Insn, InsnArgMut as IAM};
use themelios::scena::code::decompile::{TreeInsn, decompile, recompile};
use themelios::scena::ed7::Scena;
use themelios::tables::quest::{ED7Quest, self};
use themelios::text::Text;
use themelios::types::{QuestId, Flag};
use visit::VisitMut;

mod visit;
mod translate;
use translate::*;

macro_rules! f {
	($p:pat $(if $e:expr)? => $v:expr) => { |_a| {
		match _a {
			$p $(if $e)? => Some($v),
			_ => None
		}
	} };
	($p:pat $(if $e:expr)? ) => { |_a| {
		match _a {
			$p $(if $e)? => true,
			_ => false
		}
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
		self.scenas.entry(name.to_owned()).or_insert_with(|| {
			let evo = scena::ed7::read(GameData::AO_EVO, &std::fs::read(self.evo_path.join(format!("{name}.bin"))).unwrap()).unwrap();
			AScena {
				main: translate(tl, &evo),
				evo,
				new_npcs: Vec::new(),
				new_lps: Vec::new(),
				new_funcs: Vec::new(),
			}
		})
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

	fn func(&mut self, idx: usize, f: impl FnOnce(AList<Vec<TreeInsn>>)) {
		let mut f1 = decompile(&self.main.functions[idx]).unwrap();
		let f2 = decompile(&self.evo.functions[idx]).unwrap();
		f(AList(&mut f1, &f2));
		self.main.functions[idx] = recompile(&f1).unwrap();
	}
}

struct AList<'a, T>(&'a mut T, &'a T);

macro_rules! alist_map {
	($e:expr; $($t:tt)*) => { {
		let x = $e;
		AList(
			x.0.iter_mut() $($t)*,
			x.1.iter() $($t)*,
		)
	} }
}

impl<'a> AList<'a, Vec<TreeInsn>> {
	#[track_caller]
	fn if_with(self, e: &Expr) -> AList<'a, Vec<(Option<Expr>, Vec<TreeInsn>)>> {
		alist_map!(self; .find_map(f!(TreeInsn::If(x) if x.iter().any(|a| a.0.as_ref() == Some(e)) => x)).unwrap())
	}

	#[track_caller]
	fn if_clause(self, e: &Expr) -> AList<'a, Vec<TreeInsn>> {
		self.if_with(e).clause(&Some(e.clone()))
	}
}

impl<'a, A: PartialEq, B> AList<'a, Vec<(A, B)>> {
	#[track_caller]
	fn clause(self, k: &A) -> AList<'a, B> {
		alist_map!(self; .find_map(|(a,b)| (a == k).then_some(b)).unwrap())
	}

	#[track_caller]
	fn copy_clause(&mut self, k: &A, tl: &mut impl Translator) where (A, B): Clone + VisitMut {
		self.0.push(translate(tl, self.1.iter().find(|a| &a.0 == k).unwrap()));
	}
}

impl<'a, T: Clone + VisitMut> AList<'a, Vec<T>> {
	#[track_caller]
	fn copy_tail(&mut self, tl: &mut impl Translator) {
		self.0.extend(self.1[self.0.len()..].iter().map(|a| translate(tl, a)))
	}
}

macro_rules! flag {
	($n:literal) => { Expr::Flag(Flag($n)) }
}

fn main() -> anyhow::Result<()> {
	use std::fs;
	use std::io::BufWriter;
	let mut ctx = Context::new(
		"./data/ao-psp/PSP_GAME/USRDIR/data/scena/",
		"./data/ao-evo/data/scena/",
		"./data/ao-gf/data_en/scena/",
		quest::read_ed7(GameData::AO, &fs::read("./data/ao-gf/data_en/text/t_quest._dt")?)?,
		quest::read_ed7(GameData::AO_EVO, &fs::read("./data/ao-evo/data/text/t_quest._dt")?)?,
	);

	// c0110 (SSS base) has some functions moved to _1, which makes this ugly.
	let s = ctx.scena("c0110");
	s.evo.functions.insert(35, vec![]);
	s.evo.functions.insert(36, vec![]);
	s.remap(&mut |a| {
		if let IAM::FuncRef(FuncRef(0, i)) = a {
			if *i >= 35 {
				*i += 2;
			}
		}
	});

	quest125(&mut ctx);
	quest158(&mut ctx);

	let outdir = Path::new("./patch");
	if outdir.exists() {
		fs::remove_dir_all(outdir)?;
	}
	let scenadir = outdir.join("scena");
	let textdir = outdir.join("text");
	fs::create_dir_all(&scenadir)?;
	fs::create_dir_all(&textdir)?;

	fs::write(textdir.join("t_quest._dt"), quest::write_ed7(GameData::AO, &ctx.quests)?)?;
	for (name, v) in &ctx.scenas {
		fs::write(scenadir.join(format!("{name}.bin")), scena::ed7::write(GameData::AO, &v.main)?)?;
	}

	// TODO do this in a better way
	fs::create_dir_all(outdir.join("ops"))?;
	fs::create_dir_all(outdir.join("map/objects"))?;
	fs::create_dir_all(outdir.join("visual"))?;
	fs::copy("./data/ao-evo/data/ops/e3210.op2", outdir.join("ops/e3210.op2"))?;
	fs::copy("./data/ao-evo/data/map/objects/e3210isu.it3", outdir.join("map/objects/e3210isu.it3"))?;
	fs::copy("./data/ao-evo/data/visual/c_vis600.itp", outdir.join("visual/c_vis600.itp"))?;

	let dumpdir = Path::new("./patch-dump");
	if dumpdir.exists() {
		fs::remove_dir_all(dumpdir)?;
	}
	fs::create_dir_all(dumpdir)?;

	for (name, v) in &ctx.scenas {
		let ctx = calmare::Context::new(BufWriter::new(fs::File::create(dumpdir.join(name))?));
		calmare::ed7::write(ctx, &v.main)?;
	}

	Ok(())
}

fn quest125(ctx: &mut Context) {
	let nil = &mut Nil;

	let tl = &mut Translate::load(include_str!("../text/quest125.txt"));
	tl.comment("t_quest");
	ctx.copy_quest(QuestId(125), tl);

	tl.comment("c1200 - Harbor District");
	let s = ctx.scena("c1200");
	s.main.chcp[19] = Some("chr/ch28100.itc".to_owned());
	s.copy_npc(31, tl); // Reins
	s.copy_func(0, 107, tl); // talk Reins
	s.func(8, |a| a.if_clause(&flag![2564]).copy_tail(nil));

	tl.comment("c1300 - IBC exterior");
	let s = ctx.scena("c1300");
	s.main.chcp.push(Some("chr/ch06000.itc".to_owned()));
	s.copy_npc(1, tl);  // Grace
	s.copy_npc(10, tl); // Shirley
	s.copy_npc(11, tl); // Sigmund
	s.copy_func(0, 9, tl); // talk Grace
	s.func(1, |a| a.if_clause(&flag![2564]).copy_tail(nil));

	tl.comment("c0490 - Neue Blanc");
	let s = ctx.scena("c0490");
	for i in 18..=24 {
		s.copy_npc(i, tl); // Wazy's patron, Grace, Woman, Man, Man, Imperial mafioso, Republic mafioso
	}
	s.copy_func(0, 15, tl);
	for i in 16..=23 {
		s.copy_func(0, i, nil);
	}
	s.func(1, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![274]), nil));

	// c0400 - Entertainment District, where you end up after the quest
	let s = ctx.scena("c0400");
	s.func(5, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![279]), nil));

	// c0110 - SSS building, quest deadline
	let s = ctx.scena("c0110");
	s.func(37, |a| {
		let i = a.0.iter().enumerate().find_map(f!((i, TreeInsn::Insn(Insn::Sc_C4Unset(_))) => i)).unwrap();
		a.0.insert(i, a.1[i].clone());
	});

	tl.comment("c1030 - Long Lao Tavern & Inn");
	let s = ctx.scena("c1030");
	s.func(3, |a| { // Make Grace and Reins not appear in the tavern while the quest is available
		let b = a.if_clause(&flag![2564]);
		let tail = b.0.split_off(b.1.len()-1);
		let Some(TreeInsn::If(xx)) = b.1.last() else { panic!() };
		b.0.push(TreeInsn::If(vec![(translate(nil, &xx[0].0), tail)]));
	});
	s.func(37, |a| { // Talk to Grace or Reins
		let (i, if_) = a.1.iter().enumerate().find_map(f!((i, TreeInsn::If(c)) => (i, c))).unwrap();
		let mut if_ = if_.clone();
		if_[0].1 = a.0.drain(i..i+if_[0].1.len()).collect();
		do_translate(tl, &mut if_[1].1);
		a.0.insert(i, TreeInsn::If(if_));
	});
}

fn quest158(ctx: &mut Context) {
	let nil = &mut Nil;

	let tl = &mut Translate::load(include_str!("../text/quest158.txt"));
	tl.comment("t_quest");
	ctx.copy_quest(QuestId(158), tl);

	tl.comment("c0100 - Central square");
	let s = ctx.scena("c0100");
	s.main.chcp.push(Some("chr/ch41600.itc".to_owned()));
	s.copy_npc(57, tl); // Uniformed man
	s.func(7, |a| {
		let b = a.if_clause(&flag![2848]);
		let tail = b.0.split_off(b.1.len()-1);
		let Some(TreeInsn::If(xx)) = b.1.last() else { panic!() };
		b.0.push(TreeInsn::If(vec![(translate(nil, &xx[0].0), tail)]));
	});
	s.func(7, |a| a.if_clause(&flag![2573]).copy_tail(nil));

	let s = ctx.scena("c0100_1");
	s.copy_func(1, 27, tl); // talk to uniformed man

	tl.comment("t3520 - Crossbell Airport");
	let s = ctx.scena("t3520");
	s.copy_npc(23, tl); // Guardsman
	s.copy_func(0, 35, tl);
	for i in 36..=45 {
		s.copy_func(0, i, nil);
	}
	s.func(1, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![275]), nil));

	tl.comment("e3210 - Arseille");
	let s = ctx.scena("e3210");
	s.func(1, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![273]), nil));
	s.copy_func(0, 14, tl);

	tl.comment("c1100 - Administrative District");
	let s = ctx.scena("c1100");
	s.evo.includes.swap(1, 2);
	s.remap(&mut |a| {
		if let IAM::FuncRef(a) = a {
			if a.0 == 1 {
				a.0 = 2;
			} else if a.0 == 2 {
				a.0 = 1;
			}
		}
	});
	s.main.includes[2] = s.evo.includes[2].clone();
	s.copy_npc(63, tl); // Princess Klaudia
	s.copy_npc(64, tl); // Senior Captain Schwarz
	s.func(7, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![276]), nil));
	let s = ctx.copy_scena("c1100_1", tl);
	s.new_funcs = (0..s.main.functions.len()).collect();
	s.remap(&mut |a| {
		if let IAM::FuncRef(a) = a {
			if a.0 == 1 {
				a.0 = 2;
			} else if a.0 == 2 {
				a.0 = 1;
			}
		}
	});

	tl.comment("c0170 - Times Department Store");
	let s = ctx.scena("c0170");
	s.copy_npc(28, tl); // Princess Klaudia
	s.copy_npc(29, tl); // Senior Captain Schwarz
	s.copy_func(0, 54, tl);
	s.copy_func(0, 55, tl);
	s.copy_func(0, 56, nil);
	s.copy_func(0, 57, nil);
	s.copy_func(0, 58, nil);
	s.func(2, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![273]), nil));

	tl.comment("c0200 - West Street");
	let s = ctx.scena("c0200");
	s.copy_npc(30, tl); // Princess Klaudia
	s.copy_npc(31, tl); // Senior Captain Schwarz
	s.copy_func(0, 84, tl);
	s.func(11, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![275]), nil));

	tl.comment("c0210 - Morges Bakery");
	let s = ctx.scena("c0210");
	s.copy_npc(9, tl); // Princess Klaudia
	s.copy_npc(10, tl); // Senior Captain Schwarz
	s.copy_func(0, 33, tl);
	s.copy_func(0, 34, tl);
	s.copy_func(0, 35, nil);
	s.func(2, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![274]), nil));

	tl.comment("c1000 - East Street");
	let s = ctx.scena("c1000");
	s.copy_npc(35, tl); // Princess Klaudia
	s.copy_npc(36, tl); // Senior Captain Schwarz
	s.copy_func(0, 48, tl);
	s.copy_func(0, 49, tl);
	s.func(8, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![282]), nil));

	tl.comment("c1400 - Downtown District");
	let s = ctx.scena("c1400");
	s.copy_npc(18, tl); // Princess Klaudia
	s.copy_npc(19, tl); // Senior Captain Schwarz
	s.copy_func(0, 54, tl);
	s.copy_func(0, 55, tl);
	s.func(4, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![276]), nil));

	tl.comment("c0400 - Entertainment District");
	let s = ctx.scena("c0400");
	s.copy_npc(15, tl); // Princess Klaudia
	s.copy_npc(16, tl); // Senior Captain Schwarz
	s.copy_func(0, 42, tl);
	s.copy_func(0, 43, nil);
	s.func(5, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![280]), nil));

	tl.comment("c0410 - Arc en Ciel");
	let s = ctx.scena("c0410");
	s.copy_npc(15, tl); // Princess Klaudia
	s.copy_npc(16, tl); // Senior Captain Schwarz
	s.copy_func(0, 38, tl);
	s.main.functions.pop();
	s.new_funcs.pop();
	s.copy_func(0, 59, tl);
	for i in 60..=68 {
		s.copy_func(0, i, nil);
	}
	s.func(5, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![275]), nil));

	tl.comment("c0420 - Arc en Ciel (??)");
	let s = ctx.scena("c0420");
	s.copy_npc(15, tl); // Princess Klaudia
	s.copy_npc(16, tl); // Senior Captain Schwarz
	s.copy_func(0, 59, tl);
	s.copy_func(0, 60, tl);
	s.func(4, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![273]), nil));

	tl.comment("e3210 - Arseille, round two");
	let s = ctx.scena("e3210");
	s.func(1, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![274]), nil));
	s.copy_func(0, 15, tl);
	s.copy_func(0, 16, nil);

	// c0110 - Special Support Section
	let s = ctx.scena("c0110");
	s.func(2, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![287]), nil));
}
