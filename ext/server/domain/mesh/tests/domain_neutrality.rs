//! Domain-neutrality integration test (#753 Flight 77).
//!
//! A single generic function `describe<D: Domain>()` is instantiated
//! against `Text` and `Mesh` without knowing anything about either.
//! The zero-edit-for-new-domain invariant is true iff this file
//! compiles and passes — no kernel, subsys, or render-pipeline
//! change is visible from here.

use {reovim_domain::Domain, reovim_domain_mesh::Mesh, reovim_domain_text::Text};

/// Generic over any [`Domain`]. The function body knows nothing
/// about text or mesh — it only uses the trait's associated-type
/// projections.
fn describe<D: Domain>() -> (&'static str, &'static str, &'static str) {
    (
        core::any::type_name::<D::Position>(),
        core::any::type_name::<D::Edit>(),
        core::any::type_name::<D::Content>(),
    )
}

#[test]
fn text_and_mesh_satisfy_domain_from_one_generic() {
    let (tp, te, tc) = describe::<Text>();
    let (mp, me, mc) = describe::<Mesh>();

    assert!(tp.contains("TextPosition"), "Text::Position = {tp}");
    assert!(te.contains("TextEdit"), "Text::Edit = {te}");
    assert!(tc.contains("String"), "Text::Content = {tc}");

    assert!(mp.contains("MeshPosition"), "Mesh::Position = {mp}");
    assert!(me.contains("MeshEdit"), "Mesh::Edit = {me}");
    assert!(mc.contains("Vec") && mc.contains("Vertex"), "Mesh::Content = {mc}");

    // The two associated-type sets must be genuinely distinct — if
    // they're not, the scaffold isn't proving anything.
    assert_ne!(tp, mp);
    assert_ne!(te, me);
    assert_ne!(tc, mc);
}
