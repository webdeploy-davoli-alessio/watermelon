use core::array;

use bytes::{Buf, Bytes};

pub(crate) fn split_spaces(mut bytes: Bytes) -> impl Iterator<Item = Bytes> {
    let mut chunks = array::from_fn::<_, 6, _>(|_| Bytes::new());
    let mut found = 0;

    for chunk in &mut chunks {
        let Some(i) = memchr::memchr2(b' ', b'\t', &bytes) else {
            if !bytes.is_empty() {
                *chunk = bytes;
                found += 1;
            }
            break;
        };

        *chunk = bytes.split_to(i);
        found += 1;

        let spaces = bytes
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        bytes.advance(spaces);
    }

    chunks.into_iter().take(found)
}
