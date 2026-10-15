use super::super::*;
use crate::native::*;
use std::cell::RefCell;
use std::rc::Rc;

fn nodes(label: &str) -> Vec<TreeNode> {
    vec![TreeNode::new("root", label).expanded(true).children([
        TreeNode::new("first", "First"),
        TreeNode::new("second", "Second"),
    ])]
}

#[test]
fn mounts_updates_and_routes_recursive_tree_nodes() {
    let invoked = Rc::new(RefCell::new(String::new()));
    let capture = Rc::clone(&invoked);
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(
        TreeView::new()
            .on_item_invoked(move |label| *capture.borrow_mut() = label)
            .nodes(nodes("Root")),
    )
    .unwrap();

    let owner = pump.root().unwrap();
    let target = Pump::<RecordingRuntime>::native_root(&pump.tree, owner).unwrap();
    assert_eq!(
        pump.runtime().tree_nodes(target),
        Some(nodes("Root").as_slice())
    );
    let revision = pump
        .event_revision(target, EventId::TreeViewItemInvoked)
        .unwrap();
    pump.queue_event(QueuedEvent::new(
        target,
        EventId::TreeViewItemInvoked,
        revision,
        EventPayload::Str("Second".to_string()),
    ));
    assert_eq!(pump.dispatch_events(), Ok(1));
    assert_eq!(&*invoked.borrow(), "Second");

    pump.update_view(
        TreeView::new()
            .on_item_invoked(|_| {})
            .nodes(nodes("Changed")),
    )
    .unwrap();
    assert_eq!(pump.root().unwrap(), owner);
    assert_eq!(
        Pump::<RecordingRuntime>::native_root(&pump.tree, owner).unwrap(),
        target
    );
    assert_eq!(
        pump.runtime().tree_nodes(target),
        Some(nodes("Changed").as_slice())
    );
}

#[test]
fn rejects_duplicate_sibling_keys_at_any_depth() {
    let mut root = Pump::new(RecordingRuntime::default());
    assert_eq!(
        root.mount_view(TreeView::new().nodes([
            TreeNode::new("duplicate", "First"),
            TreeNode::new("duplicate", "Second"),
        ])),
        Err(PumpError::DuplicateKey(Key::from("duplicate")))
    );

    let mut nested = Pump::new(RecordingRuntime::default());
    assert_eq!(
        nested.mount_view(
            TreeView::new().nodes([TreeNode::new("root", "Root").children([
                TreeNode::new("duplicate", "First"),
                TreeNode::new("duplicate", "Second"),
            ])])
        ),
        Err(PumpError::DuplicateKey(Key::from("duplicate")))
    );
}

#[test]
fn wrap_item_text_mounts_updates_and_clears() {
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(TreeView::new().wrap_item_text(true).nodes(nodes("Root")))
        .unwrap();
    let owner = pump.root().unwrap();
    let target = Pump::<RecordingRuntime>::native_root(&pump.tree, owner).unwrap();
    assert_eq!(
        pump.runtime()
            .node(target)
            .unwrap()
            .property(PropertyId::TreeViewItemTemplate),
        Some(&PropertyValue::Bool(true))
    );

    pump.update_view(TreeView::new().wrap_item_text(false).nodes(nodes("Root")))
        .unwrap();
    assert_eq!(
        pump.runtime()
            .node(target)
            .unwrap()
            .property(PropertyId::TreeViewItemTemplate),
        Some(&PropertyValue::Bool(false))
    );

    pump.update_view(TreeView::new().nodes(nodes("Root")))
        .unwrap();
    assert_eq!(
        pump.runtime()
            .node(target)
            .unwrap()
            .property(PropertyId::TreeViewItemTemplate),
        None
    );
}

#[test]
fn item_context_requests_route_the_node_label_before_an_enclosing_context_menu() {
    // The menu sits on an element enclosing the tree, as `on_item_context_requested`'s
    // doc says it must: a menu on the TreeView itself is opened by the
    // control's class handler, which marks the request handled first.
    let requested = Rc::new(RefCell::new(Vec::new()));
    let picked = Rc::new(RefCell::new(String::new()));
    let (capture, pick) = (Rc::clone(&requested), Rc::clone(&picked));
    let mut pump = Pump::new(RecordingRuntime::default());
    pump.mount_view(
        Border::new()
            .content(
                TreeView::new()
                    .on_item_context_requested(move |label| capture.borrow_mut().push(label))
                    .nodes(nodes("Root")),
            )
            .menu(Menu::new(
                [MenuItem::item("infer", "Infer")],
                move |label| *pick.borrow_mut() = label,
            )),
    )
    .unwrap();

    let owner = pump.root().unwrap();
    let border = Pump::<RecordingRuntime>::native_root(&pump.tree, owner).unwrap();
    let [target] = pump.tree.children(border).unwrap() else {
        panic!("the border holds the tree");
    };
    let target = Pump::<RecordingRuntime>::native_root(&pump.tree, *target).unwrap();
    let revision = pump
        .event_revision(target, EventId::TreeViewContextRequested)
        .unwrap();
    // A request over a node, then one from the empty space below the items.
    for label in ["Second", ""] {
        pump.queue_event(QueuedEvent::new(
            target,
            EventId::TreeViewContextRequested,
            revision,
            EventPayload::Str(label.to_string()),
        ));
    }
    assert_eq!(pump.dispatch_events(), Ok(2));
    assert_eq!(*requested.borrow(), ["Second", ""]);

    // The enclosing menu is an ordinary context flyout.
    let (_, kind, _, _) = pump.runtime().owned_menu(owner).unwrap();
    assert_eq!(*kind, OwnedMenuKind::ContextFlyout);
    pump.runtime_mut().queue_owned_click(owner, "Infer");
    assert_eq!(pump.dispatch_events(), Ok(1));
    assert_eq!(&*picked.borrow(), "Infer");
}
