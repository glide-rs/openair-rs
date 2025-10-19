use std::io::Cursor;

use criterion::{Criterion, criterion_group, criterion_main};

fn parser_benchmark(c: &mut Criterion) {
    let fixtures: [(&str, &[u8]); _] = [
        (
            "Switzerland",
            include_bytes!("../example_data/Switzerland.txt"),
        ),
        ("Germany", include_bytes!("../example_data/Germany.txt")),
        (
            "Germany_Border",
            include_bytes!("../example_data/Germany_Border.txt"),
        ),
        ("France", include_bytes!("../example_data/France.txt")),
    ];

    for (id, bytes) in fixtures {
        c.bench_function(id, |b| {
            b.iter(|| {
                let mut cursor = Cursor::new(bytes);
                openair::parse(&mut cursor).unwrap()
            });
        });
    }
}

criterion_group!(benches, parser_benchmark);
criterion_main!(benches);
