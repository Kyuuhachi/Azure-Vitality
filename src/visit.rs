use themelios::scena::code::{Insn, Code, FlatInsn, Expr, ExprTerm};
use themelios::scena::ed7;
use themelios::types::*;

pub mod func_id {
	use super::*;

	pub fn ed7scena(s: &mut ed7::Scena, f: &mut impl FnMut(&mut FuncId)) {
		for n in &mut s.npcs {
			f(&mut n.init);
			f(&mut n.talk);
		}
		for n in &mut s.triggers {
			f(&mut n.function);
		}
		for n in &mut s.look_points {
			f(&mut n.function);
		}
		for n in &mut s.entry {
			f(&mut n.init);
			f(&mut n.reinit);
		}

		for fun in &mut s.functions {
			func(fun, f)
		}
	}

	pub fn func(s: &mut Code, f: &mut impl FnMut(&mut FuncId)) {
		for i in &mut s.0 {
			flat_insn(i, f)
		}
	}

	pub fn flat_insn(i: &mut FlatInsn, f: &mut impl FnMut(&mut FuncId)) {
		match i {
			FlatInsn::Unless(e, _) => expr(e, f),
			FlatInsn::Goto(_) => {},
			FlatInsn::Switch(e, _, _) => expr(e, f),
			FlatInsn::Insn(i) => insn(i, f),
			FlatInsn::Label(_) => {},
		}
	}

	pub fn insn(i: &mut Insn, f: &mut impl FnMut(&mut FuncId)) {
		macro run {
			([$(($ident:ident $(($_n:ident $($ty:tt)*))*))*]) => {
				match i {
					$(Insn::$ident($($_n),*) => {
						$(run!($_n $($ty)*);)*
					})*
				}
			},
			($v:ident FuncId) => { f($v); },
			($v:ident Expr) => { expr($v, f); },
			($v:ident Vec<Insn>) => { for i in $v { insn(i, f) } },
			($i:ident $($t:tt)*) => {}
		}
		themelios::scena::code::introspect!(run);
	}

	pub fn expr(i: &mut Expr, f: &mut impl FnMut(&mut FuncId)) {
		for t in &mut i.0 {
			#[allow(clippy::single_match)]
			match t {
				ExprTerm::Insn(i) => insn(i, f),
				_ => {}
			}
		}
	}
}

pub mod char_id {
	use super::*;

	pub fn ed7scena(s: &mut ed7::Scena, f: &mut impl FnMut(&mut CharId)) {
		for fun in &mut s.functions {
			func(fun, f)
		}
	}

	pub fn func(s: &mut Code, f: &mut impl FnMut(&mut CharId)) {
		for i in &mut s.0 {
			flat_insn(i, f)
		}
	}

	pub fn flat_insn(i: &mut FlatInsn, f: &mut impl FnMut(&mut CharId)) {
		match i {
			FlatInsn::Unless(e, _) => expr(e, f),
			FlatInsn::Goto(_) => {},
			FlatInsn::Switch(e, _, _) => expr(e, f),
			FlatInsn::Insn(i) => insn(i, f),
			FlatInsn::Label(_) => {},
		}
	}

	pub fn insn(i: &mut Insn, f: &mut impl FnMut(&mut CharId)) {
		macro run {
			([$(($ident:ident $(($_n:ident $($ty:tt)*))*))*]) => {
				match i {
					$(Insn::$ident($($_n),*) => {
						$(run!($_n $($ty)*);)*
					})*
				}
			},
			($v:ident CharId) => { f($v); },
			($v:ident CharAttr) => { f(&mut $v.0); },
			($v:ident Expr) => { expr($v, f); },
			($v:ident Vec<Insn>) => { for i in $v { insn(i, f) } },
			($i:ident $($t:tt)*) => {}
		}
		themelios::scena::code::introspect!(run);
	}

	pub fn expr(i: &mut Expr, f: &mut impl FnMut(&mut CharId)) {
		for t in &mut i.0 {
			#[allow(clippy::single_match)]
			match t {
				ExprTerm::Insn(i) => insn(i, f),
				ExprTerm::CharAttr(v) => f(&mut v.0),
				_ => {}
			}
		}
	}
}

pub mod look_point {
	use super::*;

	pub fn ed7scena(s: &mut ed7::Scena, f: &mut impl FnMut(&mut LookPointId)) {
		for fun in &mut s.functions {
			func(fun, f)
		}
	}

	pub fn func(s: &mut Code, f: &mut impl FnMut(&mut LookPointId)) {
		for i in &mut s.0 {
			flat_insn(i, f)
		}
	}

	pub fn flat_insn(i: &mut FlatInsn, f: &mut impl FnMut(&mut LookPointId)) {
		match i {
			FlatInsn::Unless(e, _) => expr(e, f),
			FlatInsn::Goto(_) => {},
			FlatInsn::Switch(e, _, _) => expr(e, f),
			FlatInsn::Insn(i) => insn(i, f),
			FlatInsn::Label(_) => {},
		}
	}

	pub fn insn(i: &mut Insn, f: &mut impl FnMut(&mut LookPointId)) {
		macro run {
			([$(($ident:ident $(($_n:ident $($ty:tt)*))*))*]) => {
				match i {
					$(Insn::$ident($($_n),*) => {
						$(run!($_n $($ty)*);)*
					})*
				}
			},
			($v:ident LookPointId) => { f($v); },
			($v:ident Expr) => { expr($v, f); },
			($v:ident Vec<Insn>) => { for i in $v { insn(i, f) } },
			($i:ident $($t:tt)*) => {}
		}
		themelios::scena::code::introspect!(run);
	}

	pub fn expr(i: &mut Expr, f: &mut impl FnMut(&mut LookPointId)) {
		for t in &mut i.0 {
			#[allow(clippy::single_match)]
			match t {
				ExprTerm::Insn(i) => insn(i, f),
				_ => {}
			}
		}
	}
}
