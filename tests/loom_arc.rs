#[test]
fn arc() {
    ::loom::model(|| {
        let value: [u64; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let raw_arc = ::impatience::Arc::raw(value);

        let thandle0 = ::loom::thread::spawn(move || unsafe {
            ::std::mem::drop(::impatience::Arc::<[u64; 10]>::from_raw(raw_arc))
        });

        let thandle1 = ::loom::thread::spawn(move || unsafe {
            let arc = ::impatience::Arc::<[u64; 10]>::from_raw(raw_arc);
            arc.init_count(2);
            ::std::mem::drop(arc);
        });

        thandle0.join().unwrap();
        thandle1.join().unwrap();
    });
}
