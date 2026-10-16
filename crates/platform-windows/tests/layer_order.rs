use windows::core::Interface;
use zefiro_platform_windows::{compositor::Comp, gfx::Gfx};

#[test]
fn lazily_created_background_is_between_its_anchor_and_existing_content() {
    let gfx = Gfx::new().unwrap();
    let comp = Comp::new(&gfx).unwrap();
    let root = comp.compositor.CreateContainerVisual().unwrap();
    let anchor = comp.compositor.CreateSpriteVisual().unwrap();
    let content = comp.compositor.CreateSpriteVisual().unwrap();
    let background = comp.compositor.CreateContainerVisual().unwrap();
    root.Children().unwrap().InsertAtTop(&anchor).unwrap();
    root.Children().unwrap().InsertAtTop(&content).unwrap();
    root.Children().unwrap().InsertAtTop(&background).unwrap();
    comp.place_background(&root, &background.cast().unwrap(), &anchor.cast().unwrap()).unwrap();
    let children = root.Children().unwrap().First().unwrap();
    let mut order = Vec::new();
    while children.HasCurrent().unwrap() {
        order.push(children.Current().unwrap().as_raw() as usize);
        children.MoveNext().unwrap();
    }
    let position = |visual: windows::UI::Composition::Visual| order.iter().position(|raw| *raw == visual.as_raw() as usize).unwrap();
    let a = position(anchor.cast().unwrap());
    let b = position(background.cast().unwrap());
    let c = position(content.cast().unwrap());
    assert!(b > a.min(c) && b < a.max(c));
}
