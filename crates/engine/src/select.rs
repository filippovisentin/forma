//! Visibility, locking, groups and selection filters: Hide, Show, Isolate, Lock,
//! Unlock, Group, Ungroup, SelGroup, SelLast, SelCrv, SelMesh, SelPt, Invert.

use crate::edit::parse_ids;
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Document, Geometry, ObjectId};
use std::collections::BTreeSet;

/// `ids` plus every object sharing a group with one of them.
pub(crate) fn with_groups(doc: &Document, ids: &[ObjectId]) -> BTreeSet<ObjectId> {
    let groups: BTreeSet<u32> = ids
        .iter()
        .filter_map(|id| doc.object(*id).and_then(|o| o.group))
        .collect();
    let mut out: BTreeSet<ObjectId> = ids.iter().copied().collect();
    if !groups.is_empty() {
        out.extend(
            doc.objects()
                .filter(|o| o.group.is_some_and(|g| groups.contains(&g)))
                .map(|o| o.id),
        );
    }
    out
}

/// Listed `#id`s, or the selection.
fn listed_or_selected(
    ctx: &Context,
    args: &mut Args,
    cmd: &str,
) -> Result<Vec<ObjectId>, CommandError> {
    let listed = parse_ids(args.rest())?;
    if listed.is_empty() {
        return ctx.selected(cmd);
    }
    for id in &listed {
        if ctx.doc.object(*id).is_none() {
            return Err(CommandError::Invalid(format!("no object #{}", id.0)));
        }
    }
    Ok(listed)
}

simple_command!(
    Hide,
    "Hide",
    &[],
    "Hide [#id …] — hide the selected (or listed) objects"
);
impl Command for Hide {
    impl_meta!(Hide);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let ids = listed_or_selected(ctx, args, "Hide")?;
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_hidden(*id, true);
        }
        t.commit();
        for id in &ids {
            ctx.selection.remove(id);
        }
        Ok(format!("{} object(s) hidden", ids.len()))
    }
}

simple_command!(
    Show,
    "Show",
    &["Unisolate"],
    "Show — show all hidden objects"
);
impl Command for Show {
    impl_meta!(Show);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids: Vec<ObjectId> = ctx
            .doc
            .objects()
            .filter(|o| o.hidden)
            .map(|o| o.id)
            .collect();
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_hidden(*id, false);
        }
        t.commit();
        Ok(format!("{} object(s) shown", ids.len()))
    }
}

simple_command!(
    Isolate,
    "Isolate",
    &[],
    "Isolate — hide every visible object that is not selected"
);
impl Command for Isolate {
    impl_meta!(Isolate);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        ctx.selected("Isolate")?;
        let doc = &ctx.doc;
        let ids: Vec<ObjectId> = doc
            .objects()
            .filter(|o| !o.hidden && doc.layer(o.layer).visible)
            .filter(|o| !ctx.selection.contains(&o.id))
            .map(|o| o.id)
            .collect();
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_hidden(*id, true);
        }
        t.commit();
        Ok(format!("{} object(s) hidden", ids.len()))
    }
}

simple_command!(
    Lock,
    "Lock",
    &[],
    "Lock [#id …] — lock the selected (or listed) objects: visible but not selectable"
);
impl Command for Lock {
    impl_meta!(Lock);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let ids = listed_or_selected(ctx, args, "Lock")?;
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_locked(*id, true);
        }
        t.commit();
        for id in &ids {
            ctx.selection.remove(id);
        }
        Ok(format!("{} object(s) locked", ids.len()))
    }
}

simple_command!(Unlock, "Unlock", &[], "Unlock — unlock all locked objects");
impl Command for Unlock {
    impl_meta!(Unlock);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids: Vec<ObjectId> = ctx
            .doc
            .objects()
            .filter(|o| o.locked)
            .map(|o| o.id)
            .collect();
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_locked(*id, false);
        }
        t.commit();
        Ok(format!("{} object(s) unlocked", ids.len()))
    }
}

simple_command!(
    HideSwap,
    "HideSwap",
    &[],
    "HideSwap — show the hidden objects and hide the visible ones"
);
impl Command for HideSwap {
    impl_meta!(HideSwap);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let all: Vec<(ObjectId, bool)> = ctx.doc.objects().map(|o| (o.id, o.hidden)).collect();
        let mut t = ctx.doc.begin();
        for (id, hidden) in &all {
            t.set_hidden(*id, !hidden);
        }
        t.commit();
        let shown = all.iter().filter(|(_, h)| *h).count();
        ctx.selection.clear();
        Ok(format!(
            "{shown} object(s) shown, {} hidden",
            all.len() - shown
        ))
    }
}

simple_command!(
    LockSwap,
    "LockSwap",
    &[],
    "LockSwap — unlock the locked objects and lock the others"
);
impl Command for LockSwap {
    impl_meta!(LockSwap);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let all: Vec<(ObjectId, bool)> = ctx.doc.objects().map(|o| (o.id, o.locked)).collect();
        let mut t = ctx.doc.begin();
        for (id, locked) in &all {
            t.set_locked(*id, !locked);
        }
        t.commit();
        let unlocked = all.iter().filter(|(_, l)| *l).count();
        ctx.selection.clear();
        Ok(format!(
            "{unlocked} object(s) unlocked, {} locked",
            all.len() - unlocked
        ))
    }
}

simple_command!(
    SelVisible,
    "SelVisible",
    &[],
    "SelVisible — select every visible, unlocked object"
);
impl Command for SelVisible {
    impl_meta!(SelVisible);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let doc = &ctx.doc;
        let ids: Vec<ObjectId> = doc
            .objects()
            .filter(|o| doc.is_selectable(o))
            .map(|o| o.id)
            .collect();
        let n = ids.len();
        ctx.selection.extend(ids);
        Ok(format!("{n} object(s) selected"))
    }
}

simple_command!(
    Group,
    "Group",
    &["G"],
    "Group — put the selected objects in a new group"
);
impl Command for Group {
    impl_meta!(Group);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Group")?;
        let g = ctx
            .doc
            .objects()
            .filter_map(|o| o.group)
            .max()
            .map_or(1, |m| m + 1);
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_group(*id, Some(g));
        }
        t.commit();
        Ok(format!("group {g}: {} object(s)", ids.len()))
    }
}

simple_command!(
    Ungroup,
    "Ungroup",
    &["Ug"],
    "Ungroup — remove the selected objects from their groups"
);
impl Command for Ungroup {
    impl_meta!(Ungroup);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Ungroup")?;
        let mut t = ctx.doc.begin();
        let mut n = 0;
        for id in &ids {
            if t.doc().object(*id).is_some_and(|o| o.group.is_some()) {
                t.set_group(*id, None);
                n += 1;
            }
        }
        if n == 0 {
            return Err(CommandError::Invalid("no grouped objects selected".into()));
        }
        t.commit();
        Ok(format!("{n} object(s) ungrouped"))
    }
}

simple_command!(
    SelGroup,
    "SelGroup",
    &[],
    "SelGroup — add the other members of the selected objects' groups to the selection"
);
impl Command for SelGroup {
    impl_meta!(SelGroup);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("SelGroup")?;
        let all = with_groups(&ctx.doc, &ids);
        ctx.selection.extend(all);
        Ok(format!("{} selected", ctx.selection.len()))
    }
}

simple_command!(
    SelLast,
    "SelLast",
    &[],
    "SelLast — select the objects created by the last command"
);
impl Command for SelLast {
    impl_meta!(SelLast);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let doc = &ctx.doc;
        let ids: Vec<ObjectId> = ctx
            .last_created
            .iter()
            .copied()
            .filter(|id| doc.object(*id).is_some_and(|o| doc.is_selectable(o)))
            .collect();
        if ids.is_empty() {
            return Err(CommandError::Invalid("no last created objects".into()));
        }
        ctx.selection = ids.into_iter().collect();
        Ok(format!("{} selected", ctx.selection.len()))
    }
}

/// Add every selectable object matching `f` to the selection.
fn select_where(ctx: &mut Context, f: impl Fn(&Geometry) -> bool) -> CommandResult {
    let doc = &ctx.doc;
    let ids: Vec<ObjectId> = doc
        .objects()
        .filter(|o| doc.is_selectable(o) && f(&o.geometry))
        .map(|o| o.id)
        .collect();
    let n = ids.len();
    ctx.selection.extend(ids);
    Ok(format!("{n} added to the selection"))
}

simple_command!(SelCrv, "SelCrv", &[], "SelCrv — select all curves");
impl Command for SelCrv {
    impl_meta!(SelCrv);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_where(ctx, Geometry::is_curve)
    }
}

simple_command!(
    SelMesh,
    "SelMesh",
    &["SelSrf", "SelPolysrf"],
    "SelMesh — select all meshes (surfaces and solids are meshes for now)"
);
impl Command for SelMesh {
    impl_meta!(SelMesh);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_where(ctx, |g| matches!(g, Geometry::Mesh(_)))
    }
}

simple_command!(SelPt, "SelPt", &[], "SelPt — select all point objects");
impl Command for SelPt {
    impl_meta!(SelPt);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_where(ctx, |g| matches!(g, Geometry::Point(_)))
    }
}

simple_command!(
    Invert,
    "Invert",
    &["SelInvert"],
    "Invert — select every selectable object that is not selected, deselect the rest"
);
impl Command for Invert {
    impl_meta!(Invert);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let doc = &ctx.doc;
        ctx.selection = doc
            .objects()
            .filter(|o| doc.is_selectable(o) && !ctx.selection.contains(&o.id))
            .map(|o| o.id)
            .collect();
        Ok(format!("{} selected", ctx.selection.len()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hide_swap_lock_swap_sel_visible() {
        let mut e = crate::Engine::new();
        e.run_line("Line 0,0 1,0").unwrap();
        e.run_line("Line 0,1 1,1").unwrap();
        e.run_line("Line 0,2 1,2").unwrap();
        e.run_line("Hide #1").unwrap();
        e.run_line("HideSwap").unwrap();
        let hidden: Vec<u64> = e
            .doc()
            .objects()
            .filter(|o| o.hidden)
            .map(|o| o.id.0)
            .collect();
        assert_eq!(hidden, vec![2, 3]);
        e.run_line("Show").unwrap();
        e.run_line("Lock #2").unwrap();
        e.run_line("LockSwap").unwrap();
        let locked: Vec<u64> = e
            .doc()
            .objects()
            .filter(|o| o.locked)
            .map(|o| o.id.0)
            .collect();
        assert_eq!(locked, vec![1, 3]);
        e.run_line("SelVisible").unwrap();
        assert_eq!(e.ctx.selection.len(), 1);
        e.run_line("Undo").unwrap(); // the selection is not undoable, LockSwap is
        e.run_line("SelVisible").unwrap();
        assert_eq!(e.ctx.selection.len(), 2);
    }
    use crate::Engine;
    use forma_doc::ObjectId;

    fn obj(e: &Engine, id: u64) -> &forma_doc::Object {
        e.doc().object(ObjectId(id)).unwrap()
    }

    #[test]
    fn hide_show_isolate() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Circle 0,0 5").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("Hide").unwrap();
        assert!(obj(&e, 1).hidden);
        assert!(e.ctx.selection.is_empty());
        e.run_line("SelAll").unwrap();
        assert_eq!(e.ctx.selection.len(), 2);
        e.run_line("SelNone").unwrap();
        e.run_line("Select #3").unwrap();
        e.run_line("Isolate").unwrap();
        assert!(obj(&e, 2).hidden && !obj(&e, 3).hidden);
        e.run_line("Show").unwrap();
        assert!(e.doc().objects().all(|o| !o.hidden));
        e.run_line("Hide #2").unwrap();
        e.run_line("Unisolate").unwrap();
        assert!(!obj(&e, 2).hidden);
        e.run_line("Undo").unwrap();
        assert!(obj(&e, 2).hidden);
    }

    #[test]
    fn lock_unlock() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Line 0,0 0,10").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("Lock").unwrap();
        assert!(obj(&e, 1).locked);
        e.run_line("SelAll").unwrap();
        assert_eq!(e.ctx.selection.len(), 1);
        e.run_line("Invert").unwrap();
        assert!(e.ctx.selection.is_empty());
        e.run_line("Unlock").unwrap();
        e.run_line("SelAll").unwrap();
        assert_eq!(e.ctx.selection.len(), 2);
    }

    #[test]
    fn groups_select_together() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Line 0,0 0,10").unwrap();
        e.run_line("Line 5,5 9,9").unwrap();
        e.run_line("Select #1 #2").unwrap();
        e.run_line("Group").unwrap();
        assert_eq!(obj(&e, 1).group, Some(1));
        assert_eq!(obj(&e, 2).group, Some(1));
        e.run_line("SelNone").unwrap();
        e.run_line("Select #2").unwrap();
        assert_eq!(e.ctx.selection.len(), 2);
        e.run_line("SelNone").unwrap();
        e.run_line("Select #3").unwrap();
        e.run_line("Group").unwrap();
        assert_eq!(obj(&e, 3).group, Some(2));
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("Ungroup").unwrap();
        assert!(obj(&e, 1).group.is_none() && obj(&e, 2).group.is_none());
        assert!(e.run_line("Ungroup").is_err());
    }

    #[test]
    fn sel_group_last_and_filters() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Point 5,5").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelLast").unwrap();
        assert_eq!(e.ctx.selection.iter().next(), Some(&ObjectId(3)));
        e.run_line("SelNone").unwrap();
        e.run_line("SelCrv").unwrap();
        assert_eq!(e.ctx.selection.len(), 1);
        e.run_line("SelNone").unwrap();
        e.run_line("SelPt").unwrap();
        assert_eq!(e.ctx.selection.iter().next(), Some(&ObjectId(2)));
        e.run_line("SelSrf").unwrap();
        assert_eq!(e.ctx.selection.len(), 2);
        e.run_line("SelInvert").unwrap();
        assert_eq!(e.ctx.selection.iter().next(), Some(&ObjectId(1)));
        // SelGroup expands the selection to the group.
        e.run_line("SelAll").unwrap();
        e.run_line("Group").unwrap();
        e.run_line("SelNone").unwrap();
        e.ctx.selection.insert(ObjectId(2));
        e.run_line("SelGroup").unwrap();
        assert_eq!(e.ctx.selection.len(), 3);
    }
}
