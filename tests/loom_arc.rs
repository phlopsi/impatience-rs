#[test]
fn arc() {
    ::loom::model(|| {
        let arc = ::impatience::Arc::raw();
    });
}
