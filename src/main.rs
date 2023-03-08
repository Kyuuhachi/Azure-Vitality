#![feature(decl_macro, let_chains, backtrace_frames)]

use std::path::Path;

use themelios::scena::{self, Var, CharId, ForkId, FuncId};
use themelios::scena::code::{Expr, ExprTerm as E, ExprOp as Op, Insn, FlatInsn, Bytecode};
use themelios::scena::code::decompile::{TreeInsn, recompile};
use themelios::tables::{quest, name, bgm, se};
use themelios::types::*;

mod visit;
mod translate;
use translate::*;
mod common;
use common::*;

macro expr($($e:expr),*) { Expr(vec![$($e),*]) }
macro flag($i:literal) { E::Flag(Flag($i)) }
macro op($i:ident) { E::Op(Op::$i) }
macro flag_e($n:literal) { expr![flag!($n)] }

fn main() -> anyhow::Result<()> {
	use std::fs;
	use std::io::BufWriter;
	let mut ctx = Context::new(
		"./data/ao-psp/PSP_GAME/USRDIR/data/scena/",
		"./data/ao-evo/data/scena/",
		"./data/ao-gf/data_en/scena/",
		quest::read_ed7(&fs::read("./data/ao-gf/data_en/text/t_quest._dt")?)?,
		quest::read_ed7(&fs::read("./data/ao-evo/data/text/t_quest._dt")?)?,
	);

	timing(&mut ctx);

	quest125(&mut ctx);
	quest138(&mut ctx);
	quest157(&mut ctx);
	quest158(&mut ctx);
	quest159(&mut ctx);

	// TODO interactible furniture in c0120

	let outdir = Path::new("./patch");
	if outdir.exists() {
		fs::remove_dir_all(outdir)?;
	}
	fs::create_dir_all(outdir.join("data/text"))?;
	fs::create_dir_all(outdir.join("data_en/text"))?;
	fs::create_dir_all(outdir.join("data_en/scena"))?;

	fs::write(outdir.join("data_en/text/t_quest._dt"), quest::write_ed7(&ctx.quests)?)?;
	for (name, v) in &ctx.scenas {
		fs::write(outdir.join(format!("data_en/scena/{name}.bin")), scena::ed7::write(Game::Ao, &v.main)?)?;
	}

	fs::write(outdir.join("data_en/text/t_name._dt"), {
		let mut names = name::read_ed7(&fs::read("./data/ao-gf/data_en/text/t_name._dt")?)?;
		let names_evo = name::read_ed7(&fs::read("./data/ao-evo/data/text/t_name._dt")?)?;
		let mut mireille = names_evo.iter().find(|a| a.id == NameId(165)).unwrap().clone();
		mireille.name.0 = "Second Lieutenant Mireille".to_owned(); // Don't like that this is not in the tl files
		names.push(mireille);
		name::write_ed7(&names)?
	})?;

	// NISA Zero has both text/t_bgm and text_us/t_bgm, but they are identical. Better patch both.
	fs::write(outdir.join("data/text/t_bgm._dt"), {
		let mut bgms = bgm::read_ed7(&fs::read("./data/ao-gf/data/text/t_bgm._dt")?)?;
		let bgms_evo = bgm::read_ed7(&fs::read("./data/ao-evo/data/text/t_bgm._dt")?)?;
		bgms.push(bgms_evo.iter().find(|a| a.id == BgmId(4)).unwrap().clone());
		bgm::write_ed7(&bgms)?
	})?;

	// Geofront only. NISA instead has data/bgm/info.yaml
	fs::write(outdir.join("music.json"), {
		let data = fs::read_to_string("./data/ao-gf/music.json")?;
		let data = data.trim_start_matches('\u{FEFF}');
		let mut music: serde_json::Value = serde_json::from_str(data)?;
		music["files"].as_object_mut().unwrap().insert("4".into(), serde_json::json!({
			"en": "Way Of Life",
			"jp": "Way Of Life",
			"source": 7, // Trails to Azure Evolution
			"path": "ed7004.ogg",
			"enabled": false,
		}));
		music["soundtracks"]["0"]["files"].as_object_mut().unwrap().insert("4".into(), serde_json::json!({
			"loop": false,
			"path": "bgm/ed7004.ogg",
		}));
		serde_json::to_vec_pretty(&music)?
	})?;

	fs::write(outdir.join("data/text/t_se._dt"), {
		let mut se = se::read_ed7(&fs::read("./data/ao-gf/data/text/t_se._dt")?)?;
		let se_evo = se::read_ed7(&fs::read("./data/ao-evo/data/text/t_se._dt")?)?;
		se.push(se_evo.iter().find(|a| a.id == SoundId(1100)).unwrap().clone());
		se.push(se_evo.iter().find(|a| a.id == SoundId(1101)).unwrap().clone());
		se.push(se_evo.iter().find(|a| a.id == SoundId(1102)).unwrap().clone());
		se.push(se_evo.iter().find(|a| a.id == SoundId(1104)).unwrap().clone());
		se::write_ed7(&se)?
	})?;

	// TODO do this in a better way
	fs::create_dir_all(outdir.join("data/ops"))?;
	fs::create_dir_all(outdir.join("data/map/objects"))?;
	fs::create_dir_all(outdir.join("data/visual"))?;
	fs::create_dir_all(outdir.join("data/chr"))?;
	fs::create_dir_all(outdir.join("data/bgm"))?;
	fs::create_dir_all(outdir.join("data/se"))?;
	fs::copy("./data/ao-evo/data/ops/e3210.op2", outdir.join("data/ops/e3210.op2"))?;
	// For NISA, this it3 should be updated to use TEXI instead of TEXF. Think it'll still work without, though.
	fs::copy("./data/ao-evo/data/map/objects/e3210isu.it3", outdir.join("data/map/objects/e3210isu.it3"))?;
	// These two might need upscaling.
	fs::copy("./data/ao-evo/data/visual/c_vis600.itp", outdir.join("data/visual/c_vis600.itp"))?;
	fs::copy("./data/ao-evo/data/chr/ch40004.itc", outdir.join("data/chr/ch40004.itc"))?;
	// In NISA, this should be data_pc/bgm/ed7004.opus
	fs::copy("./text/ed7004.ogg", outdir.join("data/bgm/ed7004.ogg"))?;
	// And data_pc/se/ed7s1100.opus for these
	fs::copy("./text/ed7s1100.wav", outdir.join("data/se/ed7s1100.wav"))?;
	fs::copy("./text/ed7s1101.wav", outdir.join("data/se/ed7s1101.wav"))?;
	fs::copy("./text/ed7s1102.wav", outdir.join("data/se/ed7s1102.wav"))?;
	fs::copy("./text/ed7s1104.wav", outdir.join("data/se/ed7s1104.wav"))?;

	let dumpdir = Path::new("./dump");
	if dumpdir.exists() {
		fs::remove_dir_all(dumpdir)?;
	}
	fs::create_dir_all(dumpdir)?;

	for (name, v) in &ctx.scenas {
		let mut ctx = calmare::Context::new(Game::Ao, None);
		calmare::ed7::write(&mut ctx, &v.main);
		fs::write(dumpdir.join(name), ctx.finish())?;
		let mut ctx = calmare::Context::new(Game::Ao, None);
		calmare::ed7::write(&mut ctx, &v.evo);
		fs::write(dumpdir.join(format!("{name}.evo")), ctx.finish())?;
	}

	Ok(())
}

fn timing(ctx: &mut Context) {
	let nil = &mut Nil;

	let s = ctx.scena("c0110"); // SSS HQ
	// Two functions were moved to a subscript, undo that and reorder the functions to match
	s.evo.functions.insert(16, Bytecode(vec![]));
	s.evo.functions.insert(17, Bytecode(vec![]));
	visit::func_id::ed7scena(&mut s.evo, &mut |a| {
		if a.0 == 0 && a.1 >= 16 {
			a.1 += 2
		}
	});

	// Add quests 138 and 157 to quest list
	s.func(18, |a| {
		let a = alist_map!(a; .find_map(f!(TreeInsn::While(_, x) => x)).unwrap());
		let a = alist_map!(a; .find_map(f!(TreeInsn::Switch(_, x) => x)).unwrap());
		let a = a.clause(&Some(0)).if_clause(&flag_e![3074]);
		*a.0 = a.1.no_tl();
	});
	// And require those two to be taken before closing
	s.func(18, |a| {
		let a = a.if_with(&flag_e![275]);
		let (i0, i1) = a.index_of(f!((Some(Expr(a)), _) if matches!(a.as_slice(), [flag![275]])));
		a.0[i0-1] = a.1[i1-1].no_tl();
	});
	s.func(19, |a| {
		let a = a.if_clause(&flag_e![3074]);
		*a.0 = a.1.no_tl();
	});

	// quest125 deadline
	s.func(37, |a| {
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::Sc_C4Unset(_))));
		a.0.insert(i0, a.1[i1].no_tl());
	});

	let s = ctx.scena("c011b"); // SSS HQ, night
	// Also add 138 and 157 to quest list at night
	s.func(25, |a| {
		let a = alist_map!(a; .find_map(f!(TreeInsn::While(_, x) => x)).unwrap());
		let a = alist_map!(a; .find_map(f!(TreeInsn::Switch(_, x) => x)).unwrap());
		let a = a.clause(&Some(0)).if_clause(&flag_e![3074]);
		*a.0 = a.1.no_tl();
	});

	let s = ctx.scena("c0100"); // Central Square
	// quest138 and 157 deadline
	s.func(49, |a| {
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::ItemRemove(..))));
		a.0.insert(i0, a.1[i1-1].no_tl()); // quest138
		a.0.insert(i0, a.1[i1-2].no_tl()); // quest157
	});

	let s = ctx.scena("c1500"); // Orchis Tower exterior
	// There is a check for whether you have any outstanding quests before entering Orchis Tower for the conference; add quest158 to that check
	s.func(58, |a| {
		let a = alist_map!(a; .find_map(f!(TreeInsn::If(x) => x)).unwrap());
		a.0[0].0 = a.1[0].0.no_tl();
	});

	let s = ctx.scena("c1510"); // Orchis Tower interior (?)
	// quest158 deadline
	s.func(42, |a| {
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::_1B(..))));
		a.0.insert(i0, a.1[i1-1].no_tl());
	});

	let s = ctx.scena("m4200"); // Azure Wetland?
	// quest159 termintion, and log entry
	s.func(22, |a| {
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::Sc_C4Unset(_))));
		a.0.insert(i0, a.1[i1-2].no_tl());
		a.0.insert(i0+1, a.1[i1-1].no_tl());
	});
}

/// Illicit Trade Stakeout
fn quest125(ctx: &mut Context) {
	let nil = &mut Nil;
	let tl = &mut Translate::load(include_str!("../text/quest125.txt"));
	ctx.copy_quest(QuestId(125), tl);

	let s = ctx.scena("c1200"); // Harbor District
	s.main.chcp[19] = fileid("chr/ch28100.itc");
	s.copy_npc(31, tl); // Reins
	s.copy_func(0, 107, tl); // talk Reins
	s.func(8, |a| a.if_clause(&flag_e![2564]).copy_tail());

	let s = ctx.scena("c1300"); // IBC Exterior
	s.main.chcp.push(fileid("chr/ch06000.itc"));
	s.copy_npc(1, tl);  // Grace
	s.copy_npc(10, tl); // Shirley
	s.copy_npc(11, tl); // Sigmund
	s.copy_func(0, 9, tl); // talk Grace
	s.func(1, |a| a.if_clause(&flag_e![2564]).copy_tail());

	let s = ctx.scena("c0490"); // Neue Blanc
	for i in 18..=24 {
		s.copy_npc(i, tl); // Wazy's patron, Grace, Woman, Man, Man, Imperial mafioso, Republic mafioso
	}
	s.copy_func(0, 15, tl);
	for i in 16..=23 {
		s.copy_func(0, i, nil);
	}
	s.func(1, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![274])));

	// c0400 - Entertainment District, where you end up after the quest
	let s = ctx.scena("c0400");
	s.func(5, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![279])));

	let s = ctx.scena("c1030"); // Long Lao Tavern & Inn
	s.func(3, |a| { // Make Grace and Reins not appear in the tavern while the quest is available
		let b = a.if_clause(&flag_e![2564]);
		let tail = b.0.split_off(b.1.len()-1);
		let Some(TreeInsn::If(xx)) = b.1.last() else { panic!() };
		b.0.push(TreeInsn::If(vec![(xx[0].0.no_tl(), tail)]));
	});
	s.func(37, |a| { // Talk to Grace or Reins
		let (i, if_) = a.1.iter().enumerate().find_map(f!((i, TreeInsn::If(c)) => (i, c))).unwrap();
		let mut if_ = if_.clone();
		if_[0].1 = a.0.drain(i..i+if_[0].1.len()).collect();
		if_[1].1.translate(tl);
		a.0.insert(i, TreeInsn::If(if_));
	});
}

/// Bringing Home the Bakery
fn quest138(ctx: &mut Context) {
	let nil = &mut Nil;
	let tl = &mut Translate::load(include_str!("../text/quest138.txt"));
	ctx.copy_quest(QuestId(138), tl);

	let s = ctx.scena("c0210"); // Morges Bakery
	s.copy_func(0, 30, tl);
	s.copy_func(0, 31, tl);
	s.copy_func(0, 32, tl);
	s.func(2, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![273])));
	s.func(12, |a| { // Talk to Morges
		let a = a.if_clause(&flag_e![3074]).if_with(&expr![flag![1], op!(Not)]);
		a.0.insert(0, a.1[0].no_tl());
	});

	let s = ctx.scena("c0200"); // West Street
	s.copy_npc(18, tl); // Morges
	for i in 20..=29 {
		s.copy_npc(i, tl);
	}
	let start = s.main.functions.len();
	s.copy_func(0, 53, tl);
	for i in 54..=83 {
		s.copy_func(0, i, nil);
	}
	s.func(11, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![274])));

	// Replace AoEvo_F0 and F1 with userspace implementations
	let timer_var = Var(0);
	let timer_func = s.main.functions.len();
	let f = &mut s.main.functions[start];
	let i = f.iter().position(f!(FlatInsn::Insn(Insn::AoEvo_F1()))).unwrap();
	f.0.splice(i..i+1, [
		FlatInsn::Insn(Insn::Var(timer_var, expr![E::Const(0), op!(Ass)])),
		FlatInsn::Insn(Insn::ForkFunc(CharId(0), ForkId(3), FuncId(0, timer_func as u16))),
	]);
	for f in &mut s.main.functions[start..] {
		for i in &mut f.0 {
			if let FlatInsn::Unless(Expr(e), _) = i
			&& let [E::Insn(i), _, op!(Lt)] = e.as_mut_slice()
			&& let Insn::AoEvo_F0() = &**i {
				e[0] = E::Var(timer_var);
			}
		}
	}

	// timer function implementation
	// the loop never stops, but it'll end when reentering the bakery, so that doesn't matter
	visit::func_id::ed7scena(&mut s.evo, &mut |a| {
		if a.0 == 0 && a.1 as usize >= timer_func {
			a.1 += 1;
		}
	});
	s.evo.functions.insert(timer_func, Bytecode(vec![]));
	s.main.functions.insert(timer_func, recompile(&[
		TreeInsn::While(expr![E::Const(1)], vec![
			TreeInsn::Insn(Insn::Var(timer_var, expr![E::Const(1), op!(AddAss)])),
			TreeInsn::Insn(Insn::Sleep(Time(33))),
		]),
		TreeInsn::Insn(Insn::Return()),
	]).unwrap());
}

/// Temporary Theme Park Job, part 2
fn quest157(ctx: &mut Context) {
	let nil = &mut Nil;
	let tl = &mut Translate::load(include_str!("../text/quest157.txt"));
	ctx.copy_quest(QuestId(157), tl);

	let s = ctx.scena("c120d"); // Harbor District
	s.main.includes[0] = s.evo.includes[0];
	s.main.includes[1] = s.evo.includes[1];
	s.main.chcp[12] = s.evo.chcp[12];
	s.copy_npc(13, tl);
	for i in 18..=27 {
		s.copy_npc(i, tl);
	}
	s.func(4, |a| a.if_clause(&flag_e![3074]).copy_tail());
	s.func(4, |a| {
		// This doesn't use elif for the event flags. Doing ugly index stuff instead.
		a.0.splice(a.0.len()-2..a.0.len()-2, a.1[a.1.len()-4..a.1.len()-2].to_owned());
	});

	let s = ctx.copy_scena("c120d_1", tl);
	visit::char_id::ed7scena(&mut s.evo, &mut |a| {
		if a.0 == 21 {
			a.0 = 25
		} else if a.0 > 21 && a.0 < 25 {
			a.0 -= 1;
		}
	});

	let s = ctx.scena("t1390"); // MWL locker room
	s.copy_func(0, 6, nil);
	s.copy_func(0, 7, nil);
	for i in 8..=12 {
		s.copy_func(0, i, tl);
	}
	for i in 13..=29 {
		s.copy_func(0, i, nil);
	}
	s.func(0, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![274])));
	s.func(0, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![275])));

	let s = ctx.scena("c0130");
	s.func(46, |a| {
		// Inspecting Tio's toys, Lloyd recognizes Mishette in some cases.
		// Add this quest to that condition.
		let a = alist_map!(a; .find_map(f!(TreeInsn::If(x) => x)).unwrap());
		let a = AList(&mut a.0[0].1, &a.1[0].1);
		let a = alist_map!(a; .find_map(f!(TreeInsn::If(x) if x.len() == 2 => x)).unwrap());
		a.0[0].0 = a.1[0].0.no_tl();
	});
}

/// Introduction to Crossbell
fn quest158(ctx: &mut Context) {
	let nil = &mut Nil;
	let tl = &mut Translate::load(include_str!("../text/quest158.txt"));
	ctx.copy_quest(QuestId(158), tl);

	let s = ctx.scena("c0100"); // Central Square
	s.main.chcp.push(fileid("chr/ch41600.itc"));
	s.copy_npc(57, tl); // Uniformed man
	s.func(7, |a| {
		let b = a.if_clause(&flag_e![2848]);
		let tail = b.0.split_off(b.1.len()-1);
		let Some(TreeInsn::If(xx)) = b.1.last() else { panic!() };
		b.0.push(TreeInsn::If(vec![(xx[0].0.no_tl(), tail)]));
	});
	s.func(7, |a| a.if_clause(&flag_e![2573]).copy_tail());

	let s = ctx.scena("c0100_1");
	s.copy_func(1, 27, tl); // talk to uniformed man

	let s = ctx.scena("t3520"); // Crossbell Airport
	s.copy_npc(23, tl); // Guardsman
	s.copy_func(0, 35, tl);
	for i in 36..=45 {
		s.copy_func(0, i, nil);
	}
	s.func(1, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![275])));

	let s = ctx.scena("e3210"); // Arseille
	s.func(1, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![273])));
	s.copy_func(0, 14, tl);

	let s = ctx.scena("c1100"); // Administrative District
	s.evo.includes.swap(1, 2);
	visit::func_id::ed7scena(&mut s.evo, &mut |a| {
		if a.0 == 1 {
			a.0 = 2;
		} else if a.0 == 2 {
			a.0 = 1;
		}
	});
	s.main.includes[2] = s.evo.includes[2];
	s.copy_npc(63, tl); // Princess Klaudia
	s.copy_npc(64, tl); // Senior Captain Schwarz
	s.func(7, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![276])));
	let s = ctx.copy_scena("c1100_1", tl);
	visit::func_id::ed7scena(&mut s.evo, &mut |a| {
		if a.0 == 1 {
			a.0 = 2;
		} else if a.0 == 2 {
			a.0 = 1;
		}
	});

	let s = ctx.scena("c0170"); // Times Department Store
	s.copy_npc(28, tl); // Princess Klaudia
	s.copy_npc(29, tl); // Senior Captain Schwarz
	s.copy_func(0, 54, tl);
	s.copy_func(0, 55, tl);
	s.copy_func(0, 56, nil);
	s.copy_func(0, 57, nil);
	s.copy_func(0, 58, nil);
	s.func(2, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![273])));

	let s = ctx.scena("c0200"); // West Street
	s.copy_npc(30, tl); // Princess Klaudia
	s.copy_npc(31, tl); // Senior Captain Schwarz
	s.copy_func(0, 85, tl); // it's normally 84 but I add one for the singing
	s.func(11, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![275])));

	let s = ctx.scena("c0210"); // Morges Bakery
	s.copy_npc(9, tl); // Princess Klaudia
	s.copy_npc(10, tl); // Senior Captain Schwarz
	s.copy_func(0, 33, tl);
	s.copy_func(0, 34, tl);
	s.copy_func(0, 35, nil);
	s.func(2, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![274])));

	let s = ctx.scena("c1000"); // East Street
	s.copy_npc(35, tl); // Princess Klaudia
	s.copy_npc(36, tl); // Senior Captain Schwarz
	s.copy_func(0, 48, tl);
	s.copy_func(0, 49, tl);
	s.func(8, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![282])));

	let s = ctx.scena("c1400"); // Downtown District
	s.copy_npc(18, tl); // Princess Klaudia
	s.copy_npc(19, tl); // Senior Captain Schwarz
	s.copy_func(0, 54, tl);
	s.copy_func(0, 55, tl);
	s.func(4, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![276])));

	let s = ctx.scena("c0400"); // Entertainment District
	s.copy_npc(15, tl); // Princess Klaudia
	s.copy_npc(16, tl); // Senior Captain Schwarz
	s.copy_func(0, 42, tl);
	s.copy_func(0, 43, nil);
	s.func(5, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![280])));

	let s = ctx.scena("c0410"); // Arc en Ciel
	s.copy_npc(15, tl); // Princess Klaudia
	s.copy_npc(16, tl); // Senior Captain Schwarz
	s.copy_func(0, 38, tl);
	s.main.functions.pop();
	s.new_funcs.pop();
	s.copy_func(0, 59, tl);
	for i in 60..=68 {
		s.copy_func(0, i, nil);
	}
	s.func(5, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![275])));

	let s = ctx.scena("c0420"); // Arc en Ciel stage
	s.copy_npc(15, tl); // Princess Klaudia
	s.copy_npc(16, tl); // Senior Captain Schwarz
	s.copy_func(0, 59, tl);
	s.copy_func(0, 60, tl);
	s.func(4, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![273])));

	let s = ctx.scena("e3210"); // Arseille
	s.copy_func(0, 15, tl);
	s.copy_func(0, 16, nil);
	s.func(1, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![274])));

	let s = ctx.scena("c0110"); // Special Support Section
	s.func(2, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![287])));


	// Klaudia's dialogue at Orchis is different after the quest
	let s = ctx.scena("t4100"); // Orchis Tower
	s.func(22, |a| {
		let (i, if_) = a.1.iter().enumerate().find_map(f!((i, TreeInsn::If(c)) => (i, c))).unwrap();
		let mut if_ = if_.clone();
		if_[0].1 = a.0.drain(i..i+if_[0].1.len()).collect();
		if_[1].1.translate(tl);
		a.0.insert(i, TreeInsn::If(if_));
	});
}

// Searching the Forest
fn quest159(ctx: &mut Context) {
	let nil = &mut Nil;
	let tl = &mut Translate::load(include_str!("../text/quest159.txt"));
	ctx.copy_quest(QuestId(159), tl);

	let s = ctx.scena("t2020"); // Bellguard Gate
	s.copy_func(0, 15, tl);
	s.copy_func(0, 16, tl);
	s.copy_func(0, 17, tl);
	s.func(2, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![273])));
	s.func(8, |a| {
		let a = a.if_clause(&flag_e![2848]);
		let a = alist_map!(a; .find_map(f!(TreeInsn::If(x) => x)).unwrap());
		a.0.insert(0, a.1[0].no_tl());
	});

	let s = ctx.scena("r4000"); // Knox Forest Road
	s.main.chcp[0] = fileid("chr/ch32600.itc");
	s.copy_npc(0, tl); // ミレイユ三尉, not to be confused with ミレイユ准
	s.copy_func(0, 2, nil); // Mireille animation
	s.copy_func(0, 39, tl); // event
	s.copy_func(0, 40, tl); // leaving the forest
	s.copy_func(0, 41, tl); // talk to Mireille or the rope
	s.copy_func(0, 42, nil); // fork in :39
	s.func(2, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![273])));
	s.func(2, |a| {
		a.0.insert(a.0.len()-1, a.1[a.0.len()-1].no_tl());
	});
	s.func(3, |a| { // reinit
		fn f(b: &TreeInsn) -> bool {
			let e = expr![flag![2845], flag![2848], op!(Not), op!(BoolAnd)];
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
		a.0.insert(i0, a.1[i1-1].no_tl());
	});
	s.func(8, |a| a.if_with(&expr![flag![2847], op!(Not)]).copy_tail());

	let s = ctx.scena("r4050"); // Knox Forest
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
	s.func(1, |a| a.if_with(&flag_e![272]).copy_clause(&Some(flag_e![274])));
	s.func(2, |a| {
		a.0.splice(
			a.0.len()-1..a.0.len()-1,
			a.1[a.1.len()-8..a.1.len()-1].to_vec().no_tl()
		);
	});

	let s = ctx.scena("r4060"); // Knox Forest
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

	let s = ctx.scena("r4090"); // Knox Forest
	s.copy_func(0, 73, tl);
	s.copy_func(0, 74, nil);
	s.copy_func(0, 75, nil);
	s.func(0, |a| {
		a.0.insert(1, a.1[1].no_tl());
	});

	let s = ctx.scena("t2020"); // Bellguard Gate
	s.copy_func(0, 18, tl);

	let s = ctx.scena("t2000"); // Bellguard Gate exterior
	s.func(10, |a| {
		let a = a.if_clause(&flag_e![2848]);
		let TreeInsn::If(mut if_) = a.1[0].clone() else { panic!() };
		if_[0].1.translate(tl);
		if_[1].1 = a.0.drain(..).collect();
		a.0.splice(.., [TreeInsn::If(if_)]);
	});

	// There's a log entry coupled with the termination in m4200

	let s = ctx.scena("c0120"); // SSS, upper floors (?)
	// add another log entry if failed
	s.func(43, |a| {
		let (i0, i1) = a.index_of(f!(TreeInsn::Insn(Insn::FlagSet(Flag(282)))));
		a.0.insert(i0, a.1[i1-1].no_tl());
	});
}

fn fileid(name: &str) -> FileId {
	use themelios_archive::{ED7Lookup, Lookup};
	FileId(ED7Lookup.index(name).unwrap())
}
