#![feature(decl_macro)]

use std::path::Path;

use themelios::gamedata::GameData;
use themelios::scena::{self, FuncRef};
use themelios::scena::code::{Expr, Insn, InsnArgMut as IAM};
use themelios::scena::code::decompile::TreeInsn;
use themelios::tables::{quest, name};
use themelios::types::{QuestId, Flag, NameId};

mod visit;
mod translate;
use translate::*;
mod common;
use common::*;

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

	let s = ctx.scena("c0110"); // SSS HQ
	s.evo.functions.insert(16, vec![]);
	s.evo.functions.insert(17, vec![]);
	s.remap(&mut |a| {
		if let IAM::FuncRef(FuncRef(0, i)) = a {
			if *i >= 16 {
				*i += 2;
			}
		}
	});
	s.func(18, |a| { // quest list
		let a = alist_map!(a; .find_map(f!(TreeInsn::While(_, x) => x)).unwrap());
		let a = alist_map!(a; .find_map(f!(TreeInsn::Switch(_, x) => x)).unwrap());
		let a = a.clause(&Some(0)).if_clause(&flag![3074]);
		*a.0 = a.1.clone();
	});
	s.func(18, |a| { // require quests to be taken
		let a = a.if_with(&flag![275]);
		let (i0, i1) = a.index_of(f!((Some(flag![275]), _)));
		a.0[i0-1] = a.1[i1-1].clone();
	});
	s.func(19, |a| { // require quests to be taken
		let a = a.if_clause(&flag![3074]);
		*a.0 = a.1.clone();
	});

	let s = ctx.scena("c011b"); // SSS HQ, night
	s.func(25, |a| { // quest list
		let a = alist_map!(a; .find_map(f!(TreeInsn::While(_, x) => x)).unwrap());
		let a = alist_map!(a; .find_map(f!(TreeInsn::Switch(_, x) => x)).unwrap());
		let a = a.clause(&Some(0)).if_clause(&flag![3074]);
		*a.0 = a.1.clone();
	});

	quest125(&mut ctx);
	quest157(&mut ctx);
	quest158(&mut ctx);
	quest159(&mut ctx);

	// TODO interactible furniture in c0120

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

	fs::write(textdir.join("t_name._dt"), {
		let mut names = name::read_ed7(GameData::AO, &fs::read("./data/ao-gf/data_en/text/t_name._dt")?)?;
		let names_evo = name::read_ed7(GameData::AO_EVO, &fs::read("./data/ao-evo/data/text/t_name._dt")?)?;
		let mut mireille = names_evo.iter().find(|a| a.id == NameId(165)).unwrap().clone();
		mireille.name = "Second Lieutenant Mireille".to_owned(); // Don't like that this is not in the tl files
		names.push(mireille);
		name::write_ed7(GameData::AO, &names)
	}?)?;

	// TODO do this in a better way
	fs::create_dir_all(outdir.join("ops"))?;
	fs::create_dir_all(outdir.join("map/objects"))?;
	fs::create_dir_all(outdir.join("visual"))?;
	fs::copy("./data/ao-evo/data/ops/e3210.op2", outdir.join("ops/e3210.op2"))?;
	fs::copy("./data/ao-evo/data/map/objects/e3210isu.it3", outdir.join("map/objects/e3210isu.it3"))?;
	fs::copy("./data/ao-evo/data/visual/c_vis600.itp", outdir.join("visual/c_vis600.itp"))?;

	let dumpdir = Path::new("./dump");
	if dumpdir.exists() {
		fs::remove_dir_all(dumpdir)?;
	}
	fs::create_dir_all(dumpdir)?;

	for (name, v) in &ctx.scenas {
		let ctx = calmare::Context::new(GameData::AO, BufWriter::new(fs::File::create(dumpdir.join(name))?));
		calmare::ed7::write(ctx, &v.main)?;
		let ctx = calmare::Context::new(GameData::AO, BufWriter::new(fs::File::create(dumpdir.join(format!("{name}.evo")))?));
		calmare::ed7::write(ctx, &v.evo)?;
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
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::Sc_C4Unset(_))));
		a.0.insert(i0, translate(nil, &a.1[i1]));
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

fn quest157(ctx: &mut Context) {
	let nil = &mut Nil;
	let tl = &mut Translate::load(include_str!("../text/quest157.txt"));

	tl.comment("t_quest");
	ctx.copy_quest(QuestId(157), tl);

	let s = ctx.scena("c120d");
	s.main.includes[0] = s.evo.includes[0].clone();
	s.main.includes[1] = s.evo.includes[1].clone();
	s.main.chcp[12] = s.evo.chcp[12].clone();
	s.copy_npc(13, tl);
	for i in 18..=27 {
		s.copy_npc(i, tl);
	}
	s.func(4, |a| a.if_clause(&flag![3074]).copy_tail(nil));
	s.func(4, |a| {
		// This doesn't use elif for the event flags. Doing ugly index stuff instead.
		a.0.splice(a.0.len()-2..a.0.len()-2, a.1[a.1.len()-4..a.1.len()-2].to_owned());
	});

	let s = ctx.copy_scena("c120d_1", tl);
	s.remap(&mut |a| {
		if let IAM::CharId(a) = a {
			if a.0 == 21 {
				a.0 = 25
			} else if a.0 > 21 && a.0 < 25 {
				a.0 -= 1;
			}
		}
	});

	let s = ctx.scena("t1390");
	s.copy_func(0, 6, nil);
	s.copy_func(0, 7, nil);
	for i in 8..=12 {
		s.copy_func(0, i, tl);
	}
	for i in 13..=29 {
		s.copy_func(0, i, nil);
	}
	s.func(0, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![274]), nil));
	s.func(0, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![275]), nil));

	let s = ctx.scena("c0130");
	s.func(46, |a| {
		// Inspecting Tio's toys, Lloyd recognizes Mishette
		let a = alist_map!(a; .find_map(f!(TreeInsn::If(x) => x)).unwrap());
		let a = AList(&mut a.0[0].1, &a.1[0].1);
		let a = alist_map!(a; .find_map(f!(TreeInsn::If(x) if x.len() == 2 => x)).unwrap());
		a.0[0].0 = a.1[0].0.clone();
	});

	let s = ctx.scena("c0100"); // termination
	s.func(49, |a| {
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::ItemRemove(..))));
		a.0.insert(i0, translate(nil, &a.1[i1-1])); // quest138, TODO
		a.0.insert(i0, translate(nil, &a.1[i1-2])); // quest157
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

	tl.comment("c0420 - Arc en Ciel stage");
	let s = ctx.scena("c0420");
	s.copy_npc(15, tl); // Princess Klaudia
	s.copy_npc(16, tl); // Senior Captain Schwarz
	s.copy_func(0, 59, tl);
	s.copy_func(0, 60, tl);
	s.func(4, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![273]), nil));

	tl.comment("e3210 - Arseille, round two");
	let s = ctx.scena("e3210");
	s.copy_func(0, 15, tl);
	s.copy_func(0, 16, nil);
	s.func(1, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![274]), nil));

	// c0110 - Special Support Section
	let s = ctx.scena("c0110");
	s.func(2, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![287]), nil));

	tl.comment("t4100 - Orchis Tower");
	let s = ctx.scena("t4100");
	s.func(22, |a| {
		let (i, if_) = a.1.iter().enumerate().find_map(f!((i, TreeInsn::If(c)) => (i, c))).unwrap();
		let mut if_ = if_.clone();
		if_[0].1 = a.0.drain(i..i+if_[0].1.len()).collect();
		do_translate(tl, &mut if_[1].1);
		a.0.insert(i, TreeInsn::If(if_));
	});

	// Timing stuff, Orchis Tower exterior
	let s = ctx.scena("c1500");
	s.func(58, |a| {
		// Different dialogue if you have outstanding quests, so add this to the list
		let a = alist_map!(a; .find_map(f!(TreeInsn::If(x) => x)).unwrap());
		a.0[0].0 = translate(nil, &a.1[0].0);
	});

	// c0110 - Orchis Tower interior (?), quest deadline
	let s = ctx.scena("c1510");
	s.func(42, |a| {
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::_1B(..))));
		a.0.insert(i0, translate(nil, &a.1[i1]));
	});
}

fn quest159(ctx: &mut Context) {
	let nil = &mut Nil;

	let tl = &mut Translate::load(include_str!("../text/quest159.txt"));
	tl.comment("t_quest");
	ctx.copy_quest(QuestId(159), tl);

	tl.comment("t2020 - Bellguard Gate");
	let s = ctx.scena("t2020");
	s.copy_func(0, 15, tl);
	s.copy_func(0, 16, tl);
	s.copy_func(0, 17, tl);
	s.func(2, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![273]), nil));
	s.func(8, |a| {
		let a = a.if_clause(&flag![2848]);
		let a = alist_map!(a; .find_map(f!(TreeInsn::If(x) => x)).unwrap());
		a.0.insert(0, translate(nil, &a.1[0]));
	});

	tl.comment("r4000 - Knox Forest Road");
	let s = ctx.scena("r4000");
	s.main.chcp[0] = Some("chr/ch32600.itc".to_owned());
	s.copy_npc(0, tl); // ミレイユ三尉, not to be confused with ミレイユ准
	s.copy_func(0, 2, nil); // Mireille animation
	s.copy_func(0, 39, tl); // event
	s.copy_func(0, 40, tl); // leaving the forest
	s.copy_func(0, 41, tl); // talk to Mireille or the rope
	s.copy_func(0, 42, nil); // fork in :39
	s.func(2, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![273]), nil));
	s.func(2, |a| {
		a.0.insert(a.0.len()-1, translate(nil, &a.1[a.0.len()-1]));
	});
	s.func(3, |a| { // reinit
		fn f(b: &TreeInsn) -> bool {
			let e = flag![2845].bool_and(flag![2848].not());
			if let TreeInsn::If(b) = b {
				if let Some((b, _)) = b.get(0) {
					if b.as_ref() == Some(&e) {
						return true
					}
				}
			}
			false
		}
		let (i0, i1) = a.index_of(f);
		a.0.insert(i0, translate(nil, &a.1[i1-1]));
	});
	s.func(8, |a| a.if_with(&flag![2847].not()).copy_tail(nil));

	tl.comment("r4050 - Knox Forest");
	let s = ctx.scena("r4050");
	for i in 6..=11 {
		s.copy_look_point(i);
	}
	for i in 21..=23 {
		s.copy_func(0, i, tl);
	}
	for i in 24..=29 {
		s.copy_func(0, i, nil);
	}
	s.copy_func(0, 30, tl);
	s.copy_func(0, 31, tl);
	for i in 32..=38 {
		s.copy_func(0, i, nil);
	}
	s.func(1, |a| a.if_with(&flag![272]).copy_clause(&Some(flag![274]), nil));
	s.func(2, |a| {
		a.0.splice(a.0.len()-1..a.0.len()-1, a.1[a.1.len()-8..a.1.len()-1].iter().cloned());
	});

	tl.comment("r4060 - Knox Forest");
	let s = ctx.scena("r4060");
	for i in 4..=9 {
		s.copy_look_point(i);
	}
	s.copy_func(0, 7, tl);
	for i in 8..=13 {
		s.copy_func(0, i, nil);
	}
	s.copy_func(0, 14, tl);
	s.copy_func(0, 15, tl);
	s.copy_func(0, 16, nil);
	s.func(1, |a| {
		a.0.splice(a.0.len()-1..a.0.len()-1, a.1[a.1.len()-8..a.1.len()-1].iter().cloned());
	});

	tl.comment("r4090 - Knox Forest depths");
	let s = ctx.scena("r4090");
	s.copy_func(0, 73, tl);
	s.copy_func(0, 74, nil);
	s.copy_func(0, 75, nil);
	s.func(0, |a| {
		a.0.insert(1, translate(nil, &a.1[1]));
	});

	tl.comment("t2020 - Bellguard Gate");
	let s = ctx.scena("t2020");
	s.copy_func(0, 18, tl);

	tl.comment("t2000 - Bellguard Gate exterior");
	let s = ctx.scena("t2000");
	s.func(10, |a| {
		let a = a.if_clause(&flag![2848]);
		let TreeInsn::If(mut if_) = a.1[0].clone() else { panic!() };
		do_translate(tl, &mut if_[0].1);
		if_[1].1 = a.0.drain(..).collect();
		a.0.splice(.., [TreeInsn::If(if_)]);
	});

	 // c0120 - Special Support Section, upper floors (?)
	let s = ctx.scena("c0120");
	s.func(43, |a| { // quest termination
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::FlagSet(Flag(282)))));
		a.0.insert(i0, translate(nil, &a.1[i1-1]));
	});

	let s = ctx.scena("m4200");
	s.func(22, |a| {
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::Sc_C4Unset(_))));
		a.0.insert(i0, translate(nil, &a.1[i1-2]));
		a.0.insert(i0+1, translate(nil, &a.1[i1-1]));
	});
}

#[extend::ext]
pub impl Expr {
	fn bool_and(self, b: Expr) -> Expr {
		Expr::Binop(scena::code::ExprBinop::BoolAnd, Box::new(self), Box::new(b))
	}

	fn not(self) -> Expr {
		Expr::Unop(scena::code::ExprUnop::Not, Box::new(self))
	}
}
