use themelios::scena::code::decompile::TreeInsn;
use themelios::scena::code::{FlatInsn, Expr, Insn, InsnArgMut as IAM};
use themelios::scena::ed7;
use themelios::tables::quest::ED7Quest;

pub trait VisitMut {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM));
}

impl<T: VisitMut> VisitMut for Option<T> {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		if let Some(a) = self {
			a.accept_mut(v)
		}
	}
}

impl<T: VisitMut> VisitMut for [T] {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		for a in self {
			a.accept_mut(v)
		}
	}
}

impl<T: VisitMut> VisitMut for Vec<T> {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		for a in self {
			a.accept_mut(v)
		}
	}
}

impl<A: VisitMut, B: VisitMut> VisitMut for (A, B) {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		self.0.accept_mut(v);
		self.1.accept_mut(v);
	}
}

impl VisitMut for ed7::Entry {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		accept_val_mut(IAM::FuncRef(&mut self.init), v);
		accept_val_mut(IAM::FuncRef(&mut self.reinit), v);
	}
}

impl VisitMut for ed7::Npc {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		accept_val_mut(IAM::TextTitle(&mut self.name), v);
		accept_val_mut(IAM::FuncRef(&mut self.init), v);
		accept_val_mut(IAM::FuncRef(&mut self.talk), v);
	}
}

impl VisitMut for ed7::Monster {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		accept_val_mut(IAM::BattleId(&mut self.battle), v);
	}
}

impl VisitMut for ed7::Trigger {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		accept_val_mut(IAM::FuncRef(&mut self.function), v);
	}
}

impl VisitMut for ed7::LookPoint {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		accept_val_mut(IAM::FuncRef(&mut self.function), v);
	}
}

impl VisitMut for ed7::Scena {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		self.entry.accept_mut(v);
		self.npcs.accept_mut(v);
		self.monsters.accept_mut(v);
		self.triggers.accept_mut(v);
		self.look_points.accept_mut(v);
		self.functions.accept_mut(v);
	}
}

impl VisitMut for FlatInsn {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		match self {
			FlatInsn::Unless(e, _) => e.accept_mut(v),
			FlatInsn::Goto(_) => {},
			FlatInsn::Switch(e, _, _) => e.accept_mut(v),
			FlatInsn::Insn(i) => i.accept_mut(v),
			FlatInsn::Label(_) => {},
		}
	}
}

impl VisitMut for TreeInsn {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		match self {
			TreeInsn::If(cs) => {
				for (e, b) in cs {
					if let Some(e) = e {
						e.accept_mut(v);
					}
					b.accept_mut(v);
				}
			},
			TreeInsn::Switch(e, cs) => {
				e.accept_mut(v);
				for (_, b) in cs {
					b.accept_mut(v);
				}
			},
			TreeInsn::While(e, b) => {
				e.accept_mut(v);
				b.accept_mut(v);
			},
			TreeInsn::Break => {}
			TreeInsn::Continue => {}
			TreeInsn::Insn(i) => i.accept_mut(v)
		}
	}
}

impl VisitMut for Insn {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		for a in Vec::from(self.args_mut()) {
			accept_val_mut(a, v);
		}
	}
}

impl VisitMut for Expr {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		// TODO add type inference somehow
		accept_val_mut(IAM::Expr(self), v);
		match self {
			Expr::Binop(_, l, r) => {
				l.accept_mut(v);
				r.accept_mut(v);
			}
			Expr::Unop(_, e) => {
				e.accept_mut(v);
			}
			Expr::Insn(i) => i.accept_mut(v),
			Expr::Const(_) => {}
			Expr::Flag(x) => accept_val_mut(IAM::Flag(x), v),
			Expr::Var(x) => accept_val_mut(IAM::Var(x), v),
			Expr::Attr(x) => accept_val_mut(IAM::Attr(x), v),
			Expr::CharAttr(x) => accept_val_mut(IAM::CharAttr(x), v),
			Expr::Rand => {}
			Expr::Global(x) => accept_val_mut(IAM::Global(x), v),
		}
	}
}

impl VisitMut for ED7Quest {
	fn accept_mut(&mut self, v: &mut impl FnMut(IAM)) {
		accept_val_mut(IAM::QuestId(&mut self.id), v);
		accept_val_mut(IAM::TextTitle(&mut self.name), v);
		accept_val_mut(IAM::TextTitle(&mut self.client), v);
		accept_val_mut(IAM::Text(&mut self.desc), v);
		for s in &mut self.steps {
			accept_val_mut(IAM::Text(s), v);
		}
	}
}

fn accept_val_mut(mut e: IAM, v: &mut impl FnMut(IAM)) {
	match &mut e { // Skip unspecific types
		IAM::i16(_) |
		IAM::i32(_) |
		IAM::u8(_) |
		IAM::u16(_) |
		IAM::u32(_) |
		IAM::String(_) => return,
		_ => {}
	}
	match &mut e {
		IAM::CharAttr(a) => {
			accept_val_mut(IAM::CharId(&mut a.0), v)
		}
		IAM::Fork(a) => {
			for i in a.iter_mut() {
				i.accept_mut(v);
			}
		}
		IAM::MandatoryMembers(a) => {
			for i in a.iter_mut().flatten() {
				accept_val_mut(IAM::NameId(i), v);
			}
		}
		IAM::OptionalMembers(a) => {
			for i in a.iter_mut() {
				accept_val_mut(IAM::NameId(i), v);
			}
		}
		IAM::Menu(a) => {
			for i in a.iter_mut() {
				accept_val_mut(IAM::MenuItem(i), v);
			}
		}
		IAM::QuestList(a) => {
			for i in a.iter_mut() {
				accept_val_mut(IAM::QuestId(i), v);
			}
		}
		_ => {}
	}
	v(e);
}
