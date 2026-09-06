use super::super::*;
use crate::native::*;
use std::cell::RefCell;
use std::rc::Rc;

fn target(pump: &Pump<RecordingRuntime>) -> NodeId {
    Pump::<RecordingRuntime>::native_root(&pump.tree, pump.root().unwrap()).unwrap()
}

#[test]
fn button_menu_mounts_nested_items_and_routes_labels() {
    let clicked = Rc::new(RefCell::new(Vec::new()));
    let capture = Rc::clone(&clicked);
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(
        Button::new()
            .content(TextBlock::new().text("Open"))
            .menu(Menu::new(
                [
                    MenuItem::item("new", "New"),
                    MenuItem::separator("separator"),
                    MenuItem::submenu("share", "Share", [MenuItem::item("email", "Email")]),
                ],
                move |label| capture.borrow_mut().push(label),
            )),
    )
    .unwrap();

    let owner = pump.root().unwrap();
    let (attached, kind, items, _) = pump.runtime().owned_menu(owner).unwrap();
    assert_eq!(*attached, target(&pump));
    assert_eq!(*kind, OwnedMenuKind::ButtonFlyout);
    assert_eq!(items.len(), 3);

    pump.runtime_mut().queue_owned_click(owner, "Email");
    assert_eq!(pump.dispatch_events(), Ok(1));
    assert_eq!(&*clicked.borrow(), &["Email"]);
}

#[test]
fn unchanged_menu_reconciliation_does_not_reissue_set_owned_menu() {
    // A `.menu(...)` node is reconciled on every re-render of whatever owns
    // it, not just when the menu's own content changes (e.g. Mokuroll
    // recomputes its whole context-menu item list from scratch in every
    // `view()` call). `Command::SetOwnedMenu`'s native handler unconditionally
    // tears down and rebuilds the underlying native flyout, so reissuing it on
    // a no-op reconciliation would destroy a `ContextFlyout` popup a touch/
    // mouse user currently has open on screen out from under their pending
    // tap — see TODO.md's "タッチの長押し→コンテキストメニューからのモード
    // 切り替えが効かない" entry in the Mokuroll repo for the real-world bug
    // this was found from.
    let callback = Callback::new(|_: String| {});
    let menu = || Menu::new([MenuItem::item("open", "Open")], callback.clone());
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(
        Button::new()
            .content(TextBlock::new().text("Button"))
            .menu(menu()),
    )
    .unwrap();
    let batches_before = pump.runtime().commands().len();

    pump.update_view(
        Button::new()
            .content(TextBlock::new().text("Button"))
            .menu(menu()),
    )
    .unwrap();

    // `apply_native_commands` skips calling into the runtime entirely for an
    // empty command list (`publish.rs`), so no new batch appearing here means
    // the reconciliation produced zero native commands — not just that
    // `SetOwnedMenu` specifically was absent from a (possibly empty) new
    // batch.
    assert_eq!(
        pump.runtime().commands().len(),
        batches_before,
        "reconciling an unchanged menu must not apply any native commands \
         (in particular, must not reissue SetOwnedMenu): {:?}",
        pump.runtime().commands()
    );
}

#[test]
fn changed_menu_items_still_reissue_set_owned_menu() {
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(
        Button::new()
            .content(TextBlock::new().text("Button"))
            .menu(Menu::new([MenuItem::item("open", "Open")], |_| {})),
    )
    .unwrap();

    pump.update_view(
        Button::new()
            .content(TextBlock::new().text("Button"))
            .menu(Menu::new([MenuItem::item("close", "Close")], |_| {})),
    )
    .unwrap();

    let owner = pump.root().unwrap();
    let (_, _, items, _) = pump.runtime().owned_menu(owner).unwrap();
    assert_eq!(items, &[MenuItem::item("close", "Close")]);
    let commands = pump.runtime().commands().last().unwrap();
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::SetOwnedMenu { items: Some(_), .. })),
        "reconciling a changed menu must still reissue SetOwnedMenu: {commands:?}"
    );
}

#[test]
fn menu_bar_item_uses_owned_menu_attachment() {
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(
        MenuBarItem::new()
            .title("File")
            .menu(Menu::new([MenuItem::item("exit", "Exit")], |_| {})),
    )
    .unwrap();

    let owner = pump.root().unwrap();
    let (attached, kind, items, _) = pump.runtime().owned_menu(owner).unwrap();
    assert_eq!(*attached, target(&pump));
    assert_eq!(*kind, OwnedMenuKind::MenuBarItem);
    assert_eq!(items, &[MenuItem::item("exit", "Exit")]);
}

#[test]
fn command_bar_flyout_mounts_commands_and_routes_labels() {
    let clicked = Rc::new(RefCell::new(Vec::new()));
    let capture = Rc::clone(&clicked);
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(
        Button::new()
            .content(TextBlock::new().text("Format"))
            .command_bar_flyout(CommandBarFlyout::new(
                [CommandBarCommand::button("bold", "Bold")],
                [
                    CommandBarCommand::separator("separator"),
                    CommandBarCommand::button("copy", "Copy"),
                ],
                move |label| capture.borrow_mut().push(label),
            )),
    )
    .unwrap();

    let owner = pump.root().unwrap();
    let (attached, primary, secondary, _) = pump.runtime().command_bar_flyout(owner).unwrap();
    assert_eq!(*attached, target(&pump));
    assert_eq!(primary, &[CommandBarCommand::button("bold", "Bold")]);
    assert_eq!(secondary.len(), 2);

    pump.runtime_mut().queue_owned_click(owner, "Copy");
    assert_eq!(pump.dispatch_events(), Ok(1));
    assert_eq!(&*clicked.borrow(), &["Copy"]);
}

#[test]
fn menu_target_type_change_detaches_destroys_and_reattaches_with_current_kind() {
    let menu = || Menu::new([MenuItem::item("open", "Open")], |_| {});
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(
        Button::new()
            .content(TextBlock::new().text("Button"))
            .menu(menu()),
    )
    .unwrap();
    let owner = pump.root().unwrap();
    let old_target = target(&pump);

    pump.update_view(
        DropDownButton::new()
            .content(TextBlock::new().text("Drop down"))
            .menu(menu()),
    )
    .unwrap();

    let new_target = target(&pump);
    assert_ne!(new_target, old_target);
    let (attached, kind, _, _) = pump.runtime().owned_menu(owner).unwrap();
    assert_eq!(*attached, new_target);
    assert_eq!(*kind, OwnedMenuKind::DropDownButtonFlyout);
    let commands = pump.runtime().commands().last().unwrap();
    let detach = commands
        .iter()
        .position(|command| {
            matches!(
                command,
                Command::SetOwnedMenu {
                    target,
                    items: None,
                    ..
                } if *target == old_target
            )
        })
        .unwrap();
    let destroy = commands
        .iter()
        .position(|command| *command == Command::Destroy { node: old_target })
        .unwrap();
    let attach = commands
        .iter()
        .rposition(|command| {
            matches!(
                command,
                Command::SetOwnedMenu {
                    target,
                    kind: OwnedMenuKind::DropDownButtonFlyout,
                    items: Some(_),
                    ..
                } if *target == new_target
            )
        })
        .unwrap();
    assert!(detach < destroy);
    assert!(destroy < attach);
}

#[test]
fn command_bar_flyout_component_target_change_uses_replacement_id() {
    struct First;
    struct Second;

    macro_rules! button_component {
        ($name:ident, $label:literal) => {
            impl Component for $name {
                type Message = ();
                type Input = ();

                fn create(_input: &(), _context: &ComponentContext<Self>) -> Self {
                    Self
                }

                fn update(&mut self, _message: (), _context: &ComponentContext<Self>) {}

                fn view(&self, _input: &(), _context: &mut ViewContext<Self>) -> View {
                    Button::new().content(TextBlock::new().text($label))
                }
            }
        };
    }
    button_component!(First, "First");
    button_component!(Second, "Second");

    let flyout = || CommandBarFlyout::new([CommandBarCommand::button("copy", "Copy")], [], |_| {});
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(View::component::<First>(()).command_bar_flyout(flyout()))
        .unwrap();
    let owner = pump.root().unwrap();
    let old_target = target(&pump);

    pump.update_view(View::component::<Second>(()).command_bar_flyout(flyout()))
        .unwrap();

    let new_target = target(&pump);
    assert_ne!(new_target, old_target);
    assert_eq!(
        pump.runtime().command_bar_flyout(owner).unwrap().0,
        new_target
    );
    let commands = pump.runtime().commands().last().unwrap();
    let detach = commands
        .iter()
        .position(|command| {
            matches!(
                command,
                Command::SetCommandBarFlyout {
                    target,
                    primary: None,
                    ..
                } if *target == old_target
            )
        })
        .unwrap();
    let destroy = commands
        .iter()
        .position(|command| *command == Command::Destroy { node: old_target })
        .unwrap();
    let attach = commands
        .iter()
        .rposition(|command| {
            matches!(
                command,
                Command::SetCommandBarFlyout {
                    target,
                    primary: Some(_),
                    ..
                } if *target == new_target
            )
        })
        .unwrap();
    assert!(detach < destroy);
    assert!(destroy < attach);
}
