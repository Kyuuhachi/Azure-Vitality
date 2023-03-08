use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use themelios::scena::{self, FuncId};
use themelios::scena::code::{Expr, Bytecode};
use themelios::scena::code::decompile::{TreeInsn, decompile, recompile};
use themelios::scena::ed7::Scena;
use themelios::tables::quest::ED7Quest;
use themelios::types::{QuestId, Game};

use crate::translate::{Translator, Translatable, self};
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

pub struct Context {
	psp_path:   PathBuf,
	evo_path:   PathBuf,
	gf_path:    PathBuf,
	pub scenas: HashMap<String, AScena>,
	pub quests: Vec<ED7Quest>,
	evo_quests: Vec<ED7Quest>,
	others: HashMap<PathBuf, Cow<'static, [u8]>>,
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
			others: HashMap::new(),
		}
	}

	pub fn scena(&mut self, name: &str) -> &mut AScena {
		self.scenas.entry(name.to_owned()).or_insert_with(|| {
			let mut gf = scena::ed7::read(Game::Ao, &std::fs::read(self.gf_path.join(format!("{name}.bin"))).unwrap()).unwrap();
			let psp = scena::ed7::read(Game::Ao, &std::fs::read(self.psp_path.join(format!("{name}.bin"))).unwrap()).unwrap();
			let evo = scena::ed7::read(Game::AoEvo, &std::fs::read(self.evo_path.join(format!("{name}.bin"))).unwrap()).unwrap();
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
			let evo = scena::ed7::read(Game::AoEvo, &std::fs::read(self.evo_path.join(format!("{name}.bin"))).unwrap()).unwrap();
			let new_npcs = (0..evo.npcs.len()).collect();
			let new_lps = (0..evo.look_points.len()).collect();
			let new_funcs = (0..evo.functions.len()).collect();
			let mut main = evo.clone();
			main.labels.iter_mut().flatten().for_each(|a| a.name.translate(tl));
			main.npcs.iter_mut().for_each(|a| a.name.translate(tl));
			main.functions.iter_mut().for_each(|a| a.0.translate(tl));
			AScena {
				main,
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
		q1.name = q2.name.translated(tl);
		q1.client = q2.client.translated(tl);
		q1.desc = q2.desc.translated(tl);
		q1.steps = q2.steps.translated(tl);
		q1
	}
}


fn merge_gf(gf: &Bytecode, psp: &Bytecode) -> Option<Bytecode> {
	let mut gf = gf.0.to_owned();
	let mut psp = psp.0.to_owned(); // Because I don't have any non-mut visit
	let mut extract = translate::Extract::new();
	gf.translate(&mut extract);
	let mut inject = translate::Inject::new(extract.finish());
	psp.translate(&mut inject);
	inject.finish().then_some(Bytecode(psp))
}

pub struct AScena {
	pub main: Scena,
	pub evo: Scena,

	pub new_npcs: Vec<usize>,
	pub new_lps: Vec<usize>,
	pub new_funcs: Vec<usize>,
}

impl AScena {
	pub fn copy_npc(&mut self, idx: usize, tl: &mut impl Translator) {
		let monster_start = (8+self.main.npcs.len()) as u16;
		let monster_end = (8+self.main.npcs.len()+self.main.monsters.len()) as u16;
		visit::char_id::ed7scena(&mut self.evo, &mut |a| {
			if a.0 >= monster_start && a.0 < monster_end {
				a.0 += 1;
			}
		});

		let new_idx = self.main.npcs.len();

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
		self.main.npcs.insert(new_idx, npc2);
		self.evo.npcs.insert(new_idx, npc);
		self.new_npcs.push(new_idx);
	}

	pub fn copy_func(&mut self, scp: u16, idx: usize, tl: &mut impl Translator) {
		let new_idx = self.main.functions.len();

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
			f(&mut self.main.npcs[i].init);
			f(&mut self.main.npcs[i].talk);
		}
		for &i in &self.new_lps {
			f(&mut self.main.look_points[i].function);
		}
		for &i in &self.new_funcs {
			visit::func_id::func(&mut self.main.functions[i], &mut f);
		}

		let func = self.evo.functions.remove(idx);
		self.main.functions.push(Bytecode(func.0.translated(tl)));
		self.evo.functions.insert(new_idx, func);
		self.new_funcs.push(new_idx);
	}

	pub fn copy_look_point(&mut self, idx: usize) {
		let new_idx = self.main.look_points.len();

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
